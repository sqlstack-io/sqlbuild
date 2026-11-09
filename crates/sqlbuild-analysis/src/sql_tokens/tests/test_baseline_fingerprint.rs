use crate::sql_tokens::main::baseline_fingerprint::baseline_fingerprint;
use crate::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;

const OLD_SCHEME_HASH: &str = "fingerprint-from-an-older-scheme";

struct BaselineCase {
    description: &'static str,
    current_sql: &'static str,
    recorded_definition: &'static str,
    recorded_hash: Option<&'static str>,
    keeps_recorded: bool,
}

fn current_fingerprint(sql: &str) -> String {
    compiler_query_fingerprint(sql, Some("duckdb")).expect("fingerprint")
}

#[test]
fn given_recorded_fingerprint_when_baselining_then_only_unchanged_sql_keeps_it() {
    let cases: [BaselineCase; 5] = [
        BaselineCase {
            description: "old-scheme hash of the same SQL is kept",
            current_sql: "select id, amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: Some(OLD_SCHEME_HASH),
            keeps_recorded: true,
        },
        BaselineCase {
            description: "old-scheme hash of SQL differing only in layout and case is kept",
            current_sql: "select id, amount from orders",
            recorded_definition: "SELECT id,\n       amount\n  FROM orders -- totals",
            recorded_hash: Some(OLD_SCHEME_HASH),
            keeps_recorded: true,
        },
        BaselineCase {
            description: "a real edit is still a change",
            current_sql: "select id, amount * 2 as amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: Some(OLD_SCHEME_HASH),
            keeps_recorded: false,
        },
        BaselineCase {
            description: "an empty recorded definition counts as changed",
            current_sql: "select id, amount from orders",
            recorded_definition: "",
            recorded_hash: Some(OLD_SCHEME_HASH),
            keeps_recorded: false,
        },
        BaselineCase {
            description: "a current-scheme hash stays current",
            current_sql: "select id, amount from orders",
            recorded_definition: "select id, amount from orders",
            recorded_hash: None,
            keeps_recorded: false,
        },
    ];
    for case in cases {
        let current: String = current_fingerprint(case.current_sql);
        let recorded_hash: String = case
            .recorded_hash
            .map_or_else(|| current.clone(), str::to_owned);
        let baseline: String = baseline_fingerprint(
            current.clone(),
            (case.recorded_definition, &recorded_hash),
            |sql| compiler_query_fingerprint(sql, Some("duckdb")),
        );
        let expected: &str = if case.keeps_recorded {
            OLD_SCHEME_HASH
        } else {
            &current
        };
        assert_eq!(baseline, expected, "{}", case.description);
    }
}

#[test]
fn given_unfingerprintable_recorded_definition_when_baselining_then_current_wins() {
    let current: String = current_fingerprint("select 1");
    let baseline: String =
        baseline_fingerprint(current.clone(), ("select 1", OLD_SCHEME_HASH), |_| {
            Err("tokenizer failed".to_owned())
        });
    assert_eq!(baseline, current);
}
