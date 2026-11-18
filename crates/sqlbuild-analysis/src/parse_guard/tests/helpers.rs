use polyglot_sql::DialectType;
use sqlquack_syntax::{CallMode, Dialect};

use crate::analysis_route::models::SnowflakeDialect;
use crate::analysis_route::types::NativeDialect;
use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use crate::parse_guard::main::sqlquack_parse_options::sqlquack_parse_options;

/// A query selecting `depth` nested `REPLACE` calls around one column.
pub(super) fn nested_calls(depth: usize) -> String {
    let closings: String = (0..depth)
        .map(|level| format!(", 'a{level}', 'b')"))
        .collect();
    format!(
        "SELECT {}name{closings} AS cleaned FROM races",
        "REPLACE(".repeat(depth)
    )
}

/// `work`'s result on a thread with Rust's default 2 MiB stack, as rayon and `std::thread` use.
pub(super) fn on_default_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn_scoped(scope, work)
            .expect("thread starts")
            .join()
            .expect("parse does not overflow the stack")
    })
}

/// Each parser SQLBuild guards, with whether it accepts `sql` or the error it raised.
pub(super) fn parses(sql: &str) -> Vec<(&'static str, Result<(), String>)> {
    vec![
        (
            "native Snowflake module",
            SnowflakeDialect
                .parse(sql)
                .map(|_| ())
                .map_err(|gap| gap.message),
        ),
        (
            "SQLQuack Snowflake",
            sqlquack_parse::parse_statements_with(
                sql,
                Dialect::Snowflake,
                &sqlquack_parse_options(),
                CallMode::Faithful,
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
        ),
        (
            "Polyglot Snowflake",
            polyglot_sql::parse_with_options(
                sql,
                DialectType::Snowflake,
                &polyglot_parse_options(),
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
        ),
        (
            "Polyglot DuckDB",
            polyglot_sql::parse_with_options(sql, DialectType::DuckDB, &polyglot_parse_options())
                .map(|_| ())
                .map_err(|error| error.to_string()),
        ),
    ]
}
