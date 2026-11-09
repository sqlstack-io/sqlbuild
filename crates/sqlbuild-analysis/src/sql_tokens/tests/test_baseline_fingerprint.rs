use crate::sql_tokens::main::baseline_fingerprint::baseline_fingerprint;
use crate::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;
use crate::sql_tokens::tests::test_types::BaselineFingerprintTestCase;

const OLD_SCHEME_HASH: &str = "fingerprint-from-an-older-scheme";
const DIALECT: Option<&str> = Some("duckdb");

#[test]
fn given_recorded_fingerprint_when_baselining_then_only_unchanged_sql_keeps_it()
-> Result<(), String> {
    let test_cases = [
        BaselineFingerprintTestCase {
            description: "old-scheme hash of the same SQL is kept",
            current_sql: "select id, amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: Some(OLD_SCHEME_HASH),
            recorded_fingerprint_failure: None,
            expected_matches_recorded_and_current: (true, false),
        },
        BaselineFingerprintTestCase {
            description: "old-scheme hash of SQL differing only in layout and case is kept",
            current_sql: "select id, amount from orders",
            recorded_definition: "SELECT id,\n       amount\n  FROM orders -- totals",
            recorded_hash: Some(OLD_SCHEME_HASH),
            recorded_fingerprint_failure: None,
            expected_matches_recorded_and_current: (true, false),
        },
        BaselineFingerprintTestCase {
            description: "a real edit is still a change",
            current_sql: "select id, amount * 2 as amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: Some(OLD_SCHEME_HASH),
            recorded_fingerprint_failure: None,
            expected_matches_recorded_and_current: (false, true),
        },
        BaselineFingerprintTestCase {
            description: "an empty recorded definition counts as changed",
            current_sql: "select id, amount from orders",
            recorded_definition: "",
            recorded_hash: Some(OLD_SCHEME_HASH),
            recorded_fingerprint_failure: None,
            expected_matches_recorded_and_current: (false, true),
        },
        BaselineFingerprintTestCase {
            description: "a current-scheme hash stays current",
            current_sql: "select id, amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: None,
            recorded_fingerprint_failure: None,
            expected_matches_recorded_and_current: (false, true),
        },
        BaselineFingerprintTestCase {
            description: "a recorded definition that cannot be fingerprinted leaves the current hash",
            current_sql: "select 1",
            recorded_definition: "select 1",
            recorded_hash: Some(OLD_SCHEME_HASH),
            recorded_fingerprint_failure: Some("tokenizer failed"),
            expected_matches_recorded_and_current: (false, true),
        },
    ];
    for test_case in test_cases {
        let current: String = compiler_query_fingerprint(test_case.current_sql, DIALECT)?;
        let recorded_hash: String = test_case
            .recorded_hash
            .map_or_else(|| current.clone(), str::to_owned);
        let baseline: String = baseline_fingerprint(
            current.clone(),
            (test_case.recorded_definition, &recorded_hash),
            |sql| {
                test_case.recorded_fingerprint_failure.map_or_else(
                    || compiler_query_fingerprint(sql, DIALECT),
                    |failure| Err(failure.to_owned()),
                )
            },
        );
        assert_eq!(
            (baseline == OLD_SCHEME_HASH, baseline == current),
            test_case.expected_matches_recorded_and_current,
            "{}",
            test_case.description
        );
    }
    Ok(())
}
