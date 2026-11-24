use crate::compile_usages::_helpers::distinct_positions::distinct_positions;
use crate::compile_usages::_helpers::merged_dependencies::merged_dependencies;
use crate::compile_usages::main::schema_scope_usages::schema_scope_usages;
use crate::compile_usages::models::{DependencySource, SchemaUsages};
use crate::compile_usages::tests::helpers::{
    hidden_enum_records, hidden_schema_records, sales_schemas, visible_enum_records,
    visible_schema_records,
};
use crate::compile_usages::tests::test_types::{
    DistinctPositionsTestCase, MergedDependenciesTestCase, SchemaScopeUsagesTestCase,
};

#[test]
fn given_schemas_when_collecting_scope_usages_then_python_usages_and_s006_hold() {
    let test_cases = [
        SchemaScopeUsagesTestCase {
            description: "a visible parent and enum are used without diagnostics",
            schemas: sales_schemas,
            schema_records: visible_schema_records,
            enum_records: visible_enum_records,
            expected_usages: &[(0, 1), (0, 2)],
            expected_messages: &[],
        },
        SchemaScopeUsagesTestCase {
            description: "a hidden parent and enum are still used and each reports S006",
            schemas: sales_schemas,
            schema_records: hidden_schema_records,
            enum_records: hidden_enum_records,
            expected_usages: &[(0, 1), (0, 2)],
            expected_messages: &[
                "Schema 'base_schema' is not visible from 'sql/models/sales/schemas/orders.yml'. It is defined at sql/models/finance/schemas/base.yml with inherited scope owned by 'sql/models/finance'",
                "Schema 'orders_schema' in sql/models/sales/schemas/orders.yml column 'status' uses enum 'order_status', which is not visible from the schema's location. The enum is defined at sql/models/finance/enums/status.yml with local scope owned by 'sql/models/finance'",
            ],
        },
    ];
    for test_case in test_cases {
        let scan: SchemaUsages = schema_scope_usages(
            &(test_case.schemas)(),
            &(test_case.schema_records)(),
            &(test_case.enum_records)(),
        )
        .expect("valid paths");

        let messages = scan
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            (scan.usages.as_slice(), messages.as_slice()),
            (test_case.expected_usages, test_case.expected_messages),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_declarations_when_merging_usages_then_own_then_consumed_dependencies_dedupe() {
    let test_cases = [MergedDependenciesTestCase {
        description: "own dependencies first, then each consumed declaration once",
        declarations: &[
            ("format_amount", Some(&["round_money", "round_money"])),
            ("order_status", None),
            ("helper", Some(&[])),
        ],
        usages: &[
            (Some("format_amount"), "currency"),
            (None, "format_amount"),
            (Some("format_amount"), "round_money"),
            (Some("format_amount"), "currency"),
        ],
        expected_sources: &[
            Some(&[DependencySource::Own(0), DependencySource::Usage(0)]),
            None,
            Some(&[]),
        ],
    }];
    for test_case in test_cases {
        let declarations = test_case
            .declarations
            .iter()
            .map(|(name, own)| (*name, own.map(<[&str]>::to_vec)))
            .collect::<Vec<_>>();

        let merged = merged_dependencies(&declarations, test_case.usages);

        assert_eq!(
            merged,
            test_case
                .expected_sources
                .iter()
                .map(|sources| sources.map(<[DependencySource]>::to_vec))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_keys_when_taking_distinct_positions_then_first_occurrences_keep_order() {
    let test_cases = [DistinctPositionsTestCase {
        description: "repeats keep their first position",
        keys: &["orders", "customers", "orders", "products", "customers"],
        expected_positions: &[0, 1, 3],
    }];
    for test_case in test_cases {
        let positions: Vec<usize> = distinct_positions(test_case.keys);

        assert_eq!(
            positions, test_case.expected_positions,
            "{}",
            test_case.description
        );
    }
}
