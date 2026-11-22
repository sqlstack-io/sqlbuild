//! Cursor intrinsic calls replaced with the byte span of each replacement.

use crate::cursor_intrinsics::_helpers::scan::{
    CallScan, ScanStep, call_scan, contains_intrinsic_name, step,
};
use crate::cursor_intrinsics::models::IntrinsicSpan;
use crate::sql_lexing::main::unclosed_message::unclosed_message;
use sqlbuild_sqltext::sql_scan::main::bounded_expansions::bounded_expansions;

/// `sql` with each cursor intrinsic call replaced and each call's byte span, or the call's error.
pub(crate) fn replaced_with_spans(
    sql: &str,
    context: &str,
    replacement: impl Fn(&str) -> String,
) -> Result<(String, Vec<IntrinsicSpan>), String> {
    if !contains_intrinsic_name(sql) {
        return Ok((sql.to_owned(), Vec::new()));
    }
    let mut replaced: String = String::with_capacity(sql.len());
    let mut last: usize = 0;
    let mut spans: Vec<IntrinsicSpan> = Vec::new();
    let mut index: usize = 0;
    let mut texts: Vec<String> = Vec::new();
    while index < sql.len() {
        let (name, end): (&str, usize) = match step(sql, index) {
            ScanStep::Next(next) => {
                index = next;
                continue;
            }
            ScanStep::Unclosed(construct) => return Err(unclosed_message(context, construct)),
            ScanStep::Intrinsic { name, end } => (name, end),
        };
        let close: usize = match call_scan(sql, end) {
            CallScan::Called(close) => close,
            CallScan::NotCalled => {
                return Err(format!("{context} intrinsic {name} must be called with ()"));
            }
            CallScan::Arguments => {
                return Err(format!(
                    "{context} intrinsic {name} does not accept arguments"
                ));
            }
            CallScan::Unclosed(construct) => {
                return Err(unclosed_message(
                    &format!("{context} cursor intrinsic"),
                    construct,
                ));
            }
        };
        texts.push(replacement(name));
        spans.push(IntrinsicSpan {
            source_start: index,
            source_end: close + 1,
            output_start: 0,
            output_end: 0,
        });
        index = close + 1;
    }
    let sites: Vec<(usize, usize, &str)> = spans
        .iter()
        .zip(&texts)
        .map(|(span, text)| (span.source_start, span.source_end, text.as_str()))
        .collect();
    // A bound is a number or a quoted literal, so only `--`, which every dialect has, can form.
    for bounded in bounded_expansions(sql, &sites, None) {
        texts[bounded.index] = bounded.text;
    }
    for (span, text) in spans.iter_mut().zip(&texts) {
        replaced.push_str(&sql[last..span.source_start]);
        span.output_start = replaced.len();
        replaced.push_str(text);
        span.output_end = replaced.len();
        last = span.source_end;
    }
    replaced.push_str(&sql[last..]);
    Ok((replaced, spans))
}
