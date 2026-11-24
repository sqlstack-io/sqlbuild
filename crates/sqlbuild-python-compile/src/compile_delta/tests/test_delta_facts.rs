use std::collections::BTreeSet;

use crate::compile_delta::tests::helpers::{names, strings};
use crate::compile_delta::tests::test_types::DeltaFactTestCase;
use crate::rules::_helpers::fact::fact_replay::delta_safe_fact;

#[test]
fn given_custom_rule_fact_when_a_delta_compiles_some_models_then_only_facts_it_holds_are_safe() {
    let test_cases = [
        DeltaFactTestCase {
            description: "the SQL of a compiled model",
            key: &["sql.for_model", "sql/models/orders.sql"],
            all_sources: false,
            expected_safe: true,
        },
        DeltaFactTestCase {
            description: "the inferred columns of a compiled model",
            key: &["columns.inferred", "sql/models/orders.sql"],
            all_sources: false,
            expected_safe: true,
        },
        DeltaFactTestCase {
            description: "the SQL of a model the delta does not compile",
            key: &["sql.for_model", "sql/models/customers.sql"],
            all_sources: false,
            expected_safe: false,
        },
        DeltaFactTestCase {
            description: "the dependencies of a compiled model",
            key: &["graph.dependencies", "sql/models/orders.sql"],
            all_sources: false,
            expected_safe: false,
        },
        DeltaFactTestCase {
            description: "the text of any project file",
            key: &["tree.read_text", "sql/models/customers.sql"],
            all_sources: false,
            expected_safe: true,
        },
        DeltaFactTestCase {
            description: "the models under a folder, listed from the project's models",
            key: &["tree.resources_under", "models"],
            all_sources: false,
            expected_safe: false,
        },
        DeltaFactTestCase {
            description: "the files under a folder, listed from the filesystem",
            key: &["tree.descendants", "models"],
            all_sources: false,
            expected_safe: true,
        },
        DeltaFactTestCase {
            description: "every model",
            key: &["project.models"],
            all_sources: true,
            expected_safe: false,
        },
        DeltaFactTestCase {
            description: "every source when the delta kept them all",
            key: &["project.sources"],
            all_sources: true,
            expected_safe: true,
        },
        DeltaFactTestCase {
            description: "every source when the delta kept only those it reads",
            key: &["project.sources"],
            all_sources: false,
            expected_safe: false,
        },
    ];
    let compiled: BTreeSet<String> = names(&["sql/models/orders.sql"]);
    for test_case in test_cases {
        let safe: bool = delta_safe_fact(&strings(test_case.key), &compiled, test_case.all_sources);

        assert_eq!(safe, test_case.expected_safe, "{}", test_case.description);
    }
}
