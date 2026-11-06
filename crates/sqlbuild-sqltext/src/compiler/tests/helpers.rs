use crate::compiler::_helpers::model_headers::tokenization::{
    MAX_TOKENIZER_WORKERS, TOKENIZER_WORKER_STACK_BYTES, build_tokenizer_pool, parse_batch,
};
use crate::compiler::main::declaration_references::scan_declaration_references;
use crate::compiler::main::model_header_single_parsing::parse_one;
use crate::compiler::main::sql_interpolation::interpolate_sql;
use crate::compiler::models::CharSpan;
use crate::compiler::models::{
    AuthoredValue, DeclarationReferenceKind, DeclarationReferenceScan, DeclarationReferenceStop,
    InterpolationRead,
};
use crate::compiler::tests::test_types::{HeaderNestingTestCase, MapHost};
use crate::sql_scan::models::LexicalSyntax;
use std::collections::BTreeMap;
use std::thread;

pub(crate) fn nested_authored_headers_preserve_values_and_offsets() -> bool {
    let headers = vec![
        " columns (café (type DECIMAL(10,2))), enabled true".to_owned(),
        "constants (_countries {FR, GB}, _size constant(value 2))".to_owned(),
    ];
    let results = parse_batch(&headers).expect("worker pool builds");
    assert_eq!(results[0].2, None);
    assert_eq!(
        results[0].1.as_ref().expect("column offsets"),
        &vec![("café".to_owned(), 10, 4)]
    );
    assert_eq!(
        results[0].0,
        Some(AuthoredValue::Map(vec![
            (
                "columns".to_owned(),
                AuthoredValue::Map(vec![(
                    "café".to_owned(),
                    AuthoredValue::Map(vec![(
                        "type".to_owned(),
                        AuthoredValue::String("DECIMAL(10,2)".to_owned()),
                    )]),
                )]),
            ),
            ("enabled".to_owned(), AuthoredValue::Boolean(true)),
        ]))
    );
    assert!(matches!(
        results[1].0,
        Some(AuthoredValue::Map(ref values)) if values.len() == 1
    ));
    true
}

pub(crate) fn invalid_headers_return_each_exact_error_in_order() -> bool {
    let headers = vec![
        "schema \"analytics".to_owned(),
        "schema analytics\"mart".to_owned(),
        "schema ${ENV".to_owned(),
    ];
    let results = parse_batch(&headers).expect("worker pool builds");
    assert_eq!(
        results
            .iter()
            .map(|result| result.2.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("unterminated double-quoted string at position 7"),
            Some("unexpected double quote inside bare value at position 16; quote the whole value"),
            Some("unterminated template value at position 7"),
        ]
    );
    true
}

pub(crate) fn header_comments_are_skipped_without_shifting_offsets() -> bool {
    let headers = vec![
        "materialized table -- keep model metadata\n  columns (id ()) /* it's, (fine) */"
            .to_owned(),
        "materialized table--trailing note\n".to_owned(),
        "materialized table /* unclosed".to_owned(),
    ];

    let results = parse_batch(&headers).expect("worker pool builds");

    let AuthoredValue::Map(values) = results[0].0.as_ref().expect("header values") else {
        panic!("header values must be a map");
    };
    assert_eq!(
        values
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["materialized", "columns"]
    );
    assert_eq!(
        results[0].1.as_ref().expect("column offsets"),
        &vec![("id".to_owned(), 53, 2)]
    );
    let AuthoredValue::Map(values) = results[1].0.as_ref().expect("header values") else {
        panic!("header values must be a map");
    };
    assert_eq!(
        values
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["materialized"]
    );
    assert_eq!(
        results[2].2.as_deref(),
        Some("unterminated block comment at position 19")
    );
    true
}

pub(crate) fn nested_and_root_columns_return_only_root_offsets() -> bool {
    let headers = vec![
        "config (columns (nested (type INTEGER))), columns (top (type INTEGER))".to_owned(),
        "config (columns (nested (type INTEGER)))".to_owned(),
    ];

    let results = parse_batch(&headers).expect("worker pool builds");

    assert_eq!(
        results[0].1.as_ref().expect("column offsets"),
        &vec![("top".to_owned(), 51, 3)]
    );
    assert_eq!(
        results[1].1.as_ref().expect("column offsets"),
        &Vec::<(String, usize, usize)>::new()
    );
    true
}

pub(crate) fn batch_sizes_bound_workers_by_contract() -> bool {
    for (header_count, expected_workers) in [(0, 1), (1, 1), (3, 3), (5, MAX_TOKENIZER_WORKERS)] {
        let pool = build_tokenizer_pool(header_count).expect("worker pool builds");
        assert_eq!(pool.current_num_threads(), expected_workers);
    }
    assert_eq!(TOKENIZER_WORKER_STACK_BYTES, 16 * 1024 * 1024);
    true
}

/// Spell a scan as `kind:name[.member]@start..end, ...` plus ` | stop:<error>` when it stops.
pub(crate) fn scanned_references(sql: &str) -> Option<String> {
    spelled_scan(sql, false, None)
}

/// The same spelling of a scan under an adapter's lexical rules.
pub(crate) fn scanned_dialect_references(sql: &str, syntax: &LexicalSyntax) -> Option<String> {
    spelled_scan(sql, false, Some(syntax))
}

/// The same spelling of the usage walk, which steps past malformed references.
pub(crate) fn scanned_usages(sql: &str) -> Option<String> {
    spelled_scan(sql, true, None)
}

fn spelled_scan(sql: &str, skip_malformed: bool, syntax: Option<&LexicalSyntax>) -> Option<String> {
    let scan: DeclarationReferenceScan =
        scan_declaration_references(&[sql.to_owned()], skip_malformed, syntax).pop()?;
    let references: String = scan
        .references
        .iter()
        .map(|reference| {
            let member: String = reference
                .member
                .as_ref()
                .map_or_else(String::new, |member| format!(".{member}"));
            format!(
                "{}:{}{member}@{}..{}",
                reference.kind.keyword(),
                reference.name,
                reference.start,
                reference.end
            )
        })
        .collect::<Vec<String>>()
        .join(", ");
    let stop: String = scan.stop.map_or_else(String::new, |stop| {
        STOP_NAMES
            .iter()
            .find(|(candidate, _)| *candidate == stop)
            .map(|(_, name)| format!(" | stop:{name}"))
            .expect("every stop has a name")
    });
    Some(format!("{references}{stop}"))
}

const STOP_NAMES: [(DeclarationReferenceStop, &str); 4] = [
    (DeclarationReferenceStop::UnclosedQuote, "quote"),
    (DeclarationReferenceStop::UnclosedBlockComment, "comment"),
    (
        DeclarationReferenceStop::Malformed(DeclarationReferenceKind::Enum),
        "invalid enum",
    ),
    (
        DeclarationReferenceStop::Malformed(DeclarationReferenceKind::Constant),
        "invalid const",
    ),
];

/// Rust's default spawned-thread stack: bounded nesting must parse on any ordinary thread.
const ORDINARY_THREAD_STACK_BYTES: usize = 2 * 1024 * 1024;

/// The nesting test case's header.
pub(crate) fn nested_header(test_case: &HeaderNestingTestCase) -> String {
    format!(
        "{}{}1{}{}",
        test_case.prefix,
        test_case.open.repeat(test_case.depth),
        test_case.close.repeat(test_case.depth),
        test_case.suffix
    )
}

/// The parse error of `header`, parsed on a thread with an ordinary stack.
pub(crate) fn parse_error_on_ordinary_thread(header: String) -> Option<String> {
    thread::Builder::new()
        .stack_size(ORDINARY_THREAD_STACK_BYTES)
        .spawn(move || parse_one(&header).2)
        .expect("parser thread starts")
        .join()
        .expect("parser thread does not overflow its stack")
}

/// The error the parser reports for a container past the nesting limit at `position`.
pub(crate) fn nesting_error_at(position: usize) -> String {
    format!("values nest deeper than 256 levels at position {position}")
}

/// A host with `region = north`, `revision = 7`, `REGION=eu` and context `this`, `this.schema`.
pub(crate) fn interpolation_host(context: bool) -> MapHost {
    MapHost {
        variables: BTreeMap::from([
            ("region", Ok("north")),
            ("revision", Ok("7")),
            ("größe", Ok("large")),
            ("layout", Err("SQL variable '@@layout' is an object")),
        ]),
        environment: BTreeMap::from([("REGION", "eu"), ("ÜBER", "yes")]),
        context: context.then(|| {
            BTreeMap::from([
                ("this", Some("orders")),
                ("this.schema", Some("sales")),
                ("run_id", None),
            ])
        }),
    }
}

/// Interpolate `sql` as `models/orders.sql`: the rendered SQL or the error message.
pub(crate) fn interpolated(host: &MapHost, sql: &str) -> Result<String, String> {
    interpolate_sql(host, sql, "models/orders.sql")
        .map(|result| result.sql.unwrap_or_else(|| sql.to_owned()))
        .map_err(|failure| failure.message)
}

/// The spans and reads of interpolating `sql`.
pub(crate) fn interpolation_facts(
    host: &MapHost,
    sql: &str,
) -> (Vec<[usize; 4]>, Vec<InterpolationRead>) {
    interpolate_sql(host, sql, "models/orders.sql").map_or_else(
        |failure| (Vec::new(), failure.reads),
        |result| (result.spans.iter().map(span_parts).collect(), result.reads),
    )
}

fn span_parts(span: &CharSpan) -> [usize; 4] {
    [
        span.source_start,
        span.source_end,
        span.output_start,
        span.output_end,
    ]
}

/// Named locations from `[line, column, end_line, end_column]` rows.
pub(crate) fn locations(
    columns: Vec<(&str, [usize; 4])>,
) -> Vec<(String, crate::compiler::models::TextLocation)> {
    columns
        .into_iter()
        .map(|(name, [line, column, end_line, end_column])| {
            (
                name.to_owned(),
                crate::compiler::models::TextLocation {
                    line,
                    column,
                    end_line,
                    end_column,
                },
            )
        })
        .collect()
}

/// The string at `path`: map keys, call names, or list indexes in brackets.
pub(crate) fn string_at(value: &AuthoredValue, path: &[&str]) -> Option<String> {
    let Some((first, rest)) = path.split_first() else {
        let AuthoredValue::String(text) = value else {
            return None;
        };
        return Some(text.clone());
    };
    string_at(child(value, first)?, rest)
}

/// The value under map key `key`, or list index `key` in brackets, of `value`.
fn child<'a>(value: &'a AuthoredValue, key: &str) -> Option<&'a AuthoredValue> {
    let AuthoredValue::Map(entries) = value else {
        let AuthoredValue::List(items) = value else {
            return None;
        };
        return items.get(key.trim_matches(['[', ']']).parse::<usize>().ok()?);
    };
    entries
        .iter()
        .find(|(entry_key, _)| entry_key == key)
        .map(|(_, entry)| entry)
}

/// The string `header` parses to at `path`, or its parse error.
pub(crate) fn parsed_string(header: &str, path: &[&str]) -> Result<String, String> {
    let (value, _, error) = parse_one(header);
    error.map_or_else(
        || {
            value
                .and_then(|value| string_at(&value, path))
                .ok_or_else(|| format!("no string at {path:?}"))
        },
        Err,
    )
}
