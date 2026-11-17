use crate::graph::main::parse_selector::parse_selector;
use crate::graph::tests::helpers::{
    DIAMOND_PROJECT, FOLDER_PROJECT, TABLE_FUNCTION_PROJECT, failure, kind, names, path,
    resolved_names,
};
use crate::graph::tests::test_types::{
    ParseSelectorErrorTestCase, ParseSelectorTestCase, ProjectSelectorErrorTestCase,
    SelectedNamesTestCase,
};

#[test]
fn given_raw_selector_when_parsing_then_returns_kind_value_and_expansion() {
    let test_cases = [
        ParseSelectorTestCase {
            description: "bare model name",
            raw: "orders",
            expected_selector: kind("name", "orders", false, false),
        },
        ParseSelectorTestCase {
            description: "upstream expansion",
            raw: "+orders",
            expected_selector: kind("name", "orders", true, false),
        },
        ParseSelectorTestCase {
            description: "downstream expansion",
            raw: "orders+",
            expected_selector: kind("name", "orders", false, true),
        },
        ParseSelectorTestCase {
            description: "bidirectional expansion",
            raw: "+orders+",
            expected_selector: kind("name", "orders", true, true),
        },
        ParseSelectorTestCase {
            description: "seed typed selector",
            raw: "seed:country_codes",
            expected_selector: kind("seed", "country_codes", false, false),
        },
        ParseSelectorTestCase {
            description: "source typed selector",
            raw: "source:raw_orders",
            expected_selector: kind("source", "raw_orders", false, false),
        },
        ParseSelectorTestCase {
            description: "tag typed selector",
            raw: "tag:nightly",
            expected_selector: kind("tag", "nightly", false, false),
        },
        ParseSelectorTestCase {
            description: "path typed selector",
            raw: "path:models/staging",
            expected_selector: kind("path", "models/staging", false, false),
        },
        ParseSelectorTestCase {
            description: "typed selector with upstream expansion",
            raw: "+seed:country_codes",
            expected_selector: kind("seed", "country_codes", true, false),
        },
        ParseSelectorTestCase {
            description: "task typed selector",
            raw: "task:prepare_orders",
            expected_selector: kind("task", "prepare_orders", false, false),
        },
        ParseSelectorTestCase {
            description: "asset typed selector",
            raw: "asset:export_orders",
            expected_selector: kind("asset", "export_orders", false, false),
        },
        ParseSelectorTestCase {
            description: "loader typed selector",
            raw: "loader:load_events",
            expected_selector: kind("loader", "load_events", false, false),
        },
        ParseSelectorTestCase {
            description: "check typed selector",
            raw: "check:check_orders_export",
            expected_selector: kind("check", "check_orders_export", false, false),
        },
        ParseSelectorTestCase {
            description: "path selector",
            raw: "raw~orders",
            expected_selector: path("raw", "orders", false, false),
        },
        ParseSelectorTestCase {
            description: "path selector with upstream expansion",
            raw: "+raw~orders",
            expected_selector: path("raw", "orders", true, false),
        },
        ParseSelectorTestCase {
            description: "path selector with downstream expansion",
            raw: "raw~orders+",
            expected_selector: path("raw", "orders", false, true),
        },
        ParseSelectorTestCase {
            description: "path selector with endpoint expansion on both sides",
            raw: "+raw~orders+",
            expected_selector: path("raw", "orders", true, true),
        },
        ParseSelectorTestCase {
            description: "trailing slash is a folder",
            raw: "staging/",
            expected_selector: kind("path", "staging", false, false),
        },
        ParseSelectorTestCase {
            description: "leading slash is a folder",
            raw: "/staging",
            expected_selector: kind("path", "staging", false, false),
        },
        ParseSelectorTestCase {
            description: "nested folder with trailing slash",
            raw: "staging/orders/",
            expected_selector: kind("path", "staging/orders", false, false),
        },
        ParseSelectorTestCase {
            description: "nested folder without trailing slash",
            raw: "staging/orders",
            expected_selector: kind("path", "staging/orders", false, false),
        },
        ParseSelectorTestCase {
            description: "folder with upstream expansion",
            raw: "+staging/",
            expected_selector: kind("path", "staging", true, false),
        },
        ParseSelectorTestCase {
            description: "folder with downstream expansion",
            raw: "staging/+",
            expected_selector: kind("path", "staging", false, true),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_selector(test_case.raw),
            Ok(test_case.expected_selector),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_selector_when_parsing_then_fails_with_code() {
    let test_cases = [
        ParseSelectorErrorTestCase {
            description: "empty selector",
            raw: "",
            expected_code: "S001",
        },
        ParseSelectorErrorTestCase {
            description: "plus inside a path selector",
            raw: "a~+b",
            expected_code: "S002",
        },
        ParseSelectorErrorTestCase {
            description: "unknown typed prefix",
            raw: "unknown:value",
            expected_code: "S005",
        },
        ParseSelectorErrorTestCase {
            description: "empty value after colon",
            raw: "seed:",
            expected_code: "S006",
        },
        ParseSelectorErrorTestCase {
            description: "plus only",
            raw: "+",
            expected_code: "S004",
        },
        ParseSelectorErrorTestCase {
            description: "empty side of a path selector",
            raw: "a~",
            expected_code: "S003",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_selector(test_case.raw).map_err(|error| error.code),
            Err(test_case.expected_code),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selectors_when_resolving_then_selects_expected_names() {
    let test_cases = [
        SelectedNamesTestCase {
            description: "single model by name",
            project: DIAMOND_PROJECT,
            select: &["orders"],
            exclude: &[],
            expected_names: &["orders"],
        },
        SelectedNamesTestCase {
            description: "upstream expansion",
            project: DIAMOND_PROJECT,
            select: &["+joined"],
            exclude: &[],
            expected_names: &[
                "customers",
                "joined",
                "orders",
                "raw_customers",
                "raw_orders",
            ],
        },
        SelectedNamesTestCase {
            description: "kind selects every resource of one kind",
            project: DIAMOND_PROJECT,
            select: &["kind:source"],
            exclude: &[],
            expected_names: &["raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "+ expands its own comma part before the kind narrows it",
            project: DIAMOND_PROJECT,
            select: &["+joined,kind:source"],
            exclude: &[],
            expected_names: &["raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "downstream expansion",
            project: DIAMOND_PROJECT,
            select: &["orders+"],
            exclude: &[],
            expected_names: &["joined", "orders"],
        },
        SelectedNamesTestCase {
            description: "name glob across resource types",
            project: DIAMOND_PROJECT,
            select: &["*orders"],
            exclude: &[],
            expected_names: &["orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "downstream expansion from every glob match",
            project: DIAMOND_PROJECT,
            select: &["*orders+"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "exclusion by name glob",
            project: DIAMOND_PROJECT,
            select: &[],
            exclude: &["raw_*"],
            expected_names: &["codes", "customers", "joined", "orders"],
        },
        SelectedNamesTestCase {
            description: "union of select arguments",
            project: DIAMOND_PROJECT,
            select: &["orders", "customers"],
            exclude: &[],
            expected_names: &["customers", "orders"],
        },
        SelectedNamesTestCase {
            description: "union of space-separated tokens",
            project: DIAMOND_PROJECT,
            select: &["orders customers"],
            exclude: &[],
            expected_names: &["customers", "orders"],
        },
        SelectedNamesTestCase {
            description: "excluded model subtracted from selection",
            project: DIAMOND_PROJECT,
            select: &["orders", "customers"],
            exclude: &["customers"],
            expected_names: &["orders"],
        },
        SelectedNamesTestCase {
            description: "empty select is every resource",
            project: DIAMOND_PROJECT,
            select: &[],
            exclude: &[],
            expected_names: &[
                "codes",
                "customers",
                "joined",
                "orders",
                "raw_customers",
                "raw_orders",
            ],
        },
        SelectedNamesTestCase {
            description: "exclusion from every resource",
            project: DIAMOND_PROJECT,
            select: &[],
            exclude: &["customers"],
            expected_names: &["codes", "joined", "orders", "raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "seed by typed selector",
            project: DIAMOND_PROJECT,
            select: &["seed:codes"],
            exclude: &[],
            expected_names: &["codes"],
        },
        SelectedNamesTestCase {
            description: "source by typed selector",
            project: DIAMOND_PROJECT,
            select: &["source:raw_orders"],
            exclude: &[],
            expected_names: &["raw_orders"],
        },
        SelectedNamesTestCase {
            description: "sources by typed name glob",
            project: DIAMOND_PROJECT,
            select: &["source:raw_*"],
            exclude: &[],
            expected_names: &["raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "intersection of comma-separated tokens",
            project: DIAMOND_PROJECT,
            select: &["orders+,+joined"],
            exclude: &[],
            expected_names: &["joined", "orders"],
        },
        SelectedNamesTestCase {
            description: "bidirectional expansion",
            project: DIAMOND_PROJECT,
            select: &["+orders+"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "path selector",
            project: DIAMOND_PROJECT,
            select: &["raw_orders~joined"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "path selector with start upstream expansion",
            project: DIAMOND_PROJECT,
            select: &["+orders~joined"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "path selector with end downstream expansion",
            project: DIAMOND_PROJECT,
            select: &["raw_orders~orders+"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "path selector with endpoint expansion on both sides",
            project: DIAMOND_PROJECT,
            select: &["+raw_orders~orders+"],
            exclude: &[],
            expected_names: &["joined", "orders", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "expanded exclusion subtracted from expanded selection",
            project: DIAMOND_PROJECT,
            select: &["+joined"],
            exclude: &["orders+"],
            expected_names: &["customers", "raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "models by tag",
            project: DIAMOND_PROJECT,
            select: &["tag:nightly"],
            exclude: &[],
            expected_names: &["joined", "orders"],
        },
        SelectedNamesTestCase {
            description: "tag with downstream expansion",
            project: DIAMOND_PROJECT,
            select: &["tag:staging+"],
            exclude: &[],
            expected_names: &["joined", "orders"],
        },
        SelectedNamesTestCase {
            description: "tag intersected with a name",
            project: DIAMOND_PROJECT,
            select: &["tag:nightly,orders"],
            exclude: &[],
            expected_names: &["orders"],
        },
        SelectedNamesTestCase {
            description: "tag excluded from selection",
            project: DIAMOND_PROJECT,
            select: &["orders", "customers", "joined"],
            exclude: &["tag:nightly"],
            expected_names: &["customers"],
        },
        SelectedNamesTestCase {
            description: "consumer selection adds the table function it reads",
            project: TABLE_FUNCTION_PROJECT,
            select: &["customer_order_summary"],
            exclude: &[],
            expected_names: &["customer_order_summary", "customer_orders"],
        },
        SelectedNamesTestCase {
            description: "upstream expansion through a table function",
            project: TABLE_FUNCTION_PROJECT,
            select: &["+customer_order_summary"],
            exclude: &[],
            expected_names: &["customer_order_summary", "customer_orders", "orders"],
        },
        SelectedNamesTestCase {
            description: "table function downstream expansion reaches its consumer",
            project: TABLE_FUNCTION_PROJECT,
            select: &["customer_orders+"],
            exclude: &[],
            expected_names: &["customer_order_summary", "customer_orders"],
        },
        SelectedNamesTestCase {
            description: "folder including nested folders",
            project: FOLDER_PROJECT,
            select: &["path:models/staging"],
            exclude: &[],
            expected_names: &["stg_customers", "stg_deep", "stg_orders"],
        },
        SelectedNamesTestCase {
            description: "models root",
            project: FOLDER_PROJECT,
            select: &["path:models"],
            exclude: &[],
            expected_names: &[
                "fact_orders",
                "int_enriched",
                "stg_customers",
                "stg_deep",
                "stg_orders",
            ],
        },
        SelectedNamesTestCase {
            description: "trailing slash with explicit root",
            project: FOLDER_PROJECT,
            select: &["models/staging/"],
            exclude: &[],
            expected_names: &["stg_customers", "stg_deep", "stg_orders"],
        },
        SelectedNamesTestCase {
            description: "leading slash with explicit root",
            project: FOLDER_PROJECT,
            select: &["/models/staging"],
            exclude: &[],
            expected_names: &["stg_customers", "stg_deep", "stg_orders"],
        },
        SelectedNamesTestCase {
            description: "folder with one model",
            project: FOLDER_PROJECT,
            select: &["path:models/marts"],
            exclude: &[],
            expected_names: &["fact_orders"],
        },
        SelectedNamesTestCase {
            description: "folder with downstream expansion",
            project: FOLDER_PROJECT,
            select: &["path:models/staging+"],
            exclude: &[],
            expected_names: &[
                "fact_orders",
                "int_enriched",
                "stg_customers",
                "stg_deep",
                "stg_orders",
            ],
        },
        SelectedNamesTestCase {
            description: "folder with upstream expansion",
            project: FOLDER_PROJECT,
            select: &["+path:models/marts"],
            exclude: &[],
            expected_names: &[
                "fact_orders",
                "int_enriched",
                "raw_customers",
                "raw_orders",
                "stg_customers",
                "stg_orders",
            ],
        },
        SelectedNamesTestCase {
            description: "folder intersected with a name",
            project: FOLDER_PROJECT,
            select: &["path:models/staging,stg_orders"],
            exclude: &[],
            expected_names: &["stg_orders"],
        },
        SelectedNamesTestCase {
            description: "folder excluded from selection",
            project: FOLDER_PROJECT,
            select: &["+fact_orders"],
            exclude: &["path:models/staging"],
            expected_names: &["fact_orders", "int_enriched", "raw_customers", "raw_orders"],
        },
        SelectedNamesTestCase {
            description: "trailing slash selects a one-model folder",
            project: FOLDER_PROJECT,
            select: &["models/intermediate/"],
            exclude: &[],
            expected_names: &["int_enriched"],
        },
        SelectedNamesTestCase {
            description: "nested folder without its parent",
            project: FOLDER_PROJECT,
            select: &["path:models/staging/raw"],
            exclude: &[],
            expected_names: &["stg_deep"],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolved_names(test_case.project, test_case.select, test_case.exclude),
            Ok(names(test_case.expected_names)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unresolvable_selectors_when_resolving_then_fails_with_message() {
    let test_cases = [
        ProjectSelectorErrorTestCase {
            description: "unknown name",
            project: DIAMOND_PROJECT,
            select: &["nonexistent_model"],
            expected_error: ("S007", "unknown selector name 'nonexistent_model'", None),
        },
        ProjectSelectorErrorTestCase {
            description: "unknown node kind suggests the closest",
            project: DIAMOND_PROJECT,
            select: &["kind:modle"],
            expected_error: (
                "S007",
                "unknown node kind 'modle'",
                Some("did you mean 'model'?"),
            ),
        },
        ProjectSelectorErrorTestCase {
            description: "a Python node kind needs the Python node graph",
            project: DIAMOND_PROJECT,
            select: &["kind:task"],
            expected_error: (
                "S010",
                "selector 'kind:task' selects Python nodes; they cannot be selected here",
                None,
            ),
        },
        ProjectSelectorErrorTestCase {
            description: "name glob matching nothing",
            project: DIAMOND_PROJECT,
            select: &["missing_*"],
            expected_error: ("S007", "unknown selector pattern 'missing_*'", None),
        },
        ProjectSelectorErrorTestCase {
            description: "folder without an explicit root",
            project: FOLDER_PROJECT,
            select: &["path:nonexistent"],
            expected_error: (
                "S012",
                "path selectors require an explicit root: use 'models/' or 'python/'",
                None,
            ),
        },
        ProjectSelectorErrorTestCase {
            description: "slash folder without an explicit root",
            project: FOLDER_PROJECT,
            select: &["nonexistent/"],
            expected_error: (
                "S012",
                "path selectors require an explicit root: use 'models/' or 'python/'",
                None,
            ),
        },
        ProjectSelectorErrorTestCase {
            description: "folder with no models",
            project: FOLDER_PROJECT,
            select: &["path:models/nonexistent"],
            expected_error: (
                "S009",
                "no models found under path 'models/nonexistent'.",
                None,
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolved_names(test_case.project, test_case.select, &[]),
            Err(failure(test_case.expected_error)),
            "{}",
            test_case.description
        );
    }
}
