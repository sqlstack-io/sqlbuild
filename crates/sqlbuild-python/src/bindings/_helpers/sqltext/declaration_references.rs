//! `@enum`/`@const` reference offsets scanned natively for the model loop.

use sqlbuild_analysis::sql_macros::constants::SQL_CALL_PREFIX;
use sqlbuild_analysis::sql_macros::main::sql_macro_calls::sql_macro_calls;
use sqlbuild_sqltext::compiler::main::declaration_references::scan_declaration_references;
use sqlbuild_sqltext::compiler::models::{
    DeclarationReference, DeclarationReferenceKind, DeclarationReferenceScan,
    DeclarationReferenceStop,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The reference kind of an outer SQL macro call.
const SQL_MACRO_REFERENCE_KIND: u8 = 2;

/// One natively scanned reference: kind 0 is an enum member, 1 a constant, 2 a SQL macro call.
#[derive(Clone)]
pub(crate) struct ScannedReference {
    pub(crate) kind: u8,
    pub(crate) name: String,
    pub(crate) member: Option<String>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
/// References and stop code: unclosed quote 0, block comment 1, invalid enum 2, constant 3.
type ScanRow = (Vec<ScannedReference>, Option<u8>);

/// One SQL string's references and stop code, scanned without walking past malformed ones.
pub(crate) fn declaration_scan(sql: &str, syntax: Option<&LexicalSyntax>) -> ScanRow {
    let scanned: Vec<DeclarationReferenceScan> =
        scan_declaration_references(&[sql.to_owned()], false, syntax);
    let row: ScanRow = scanned.into_iter().next().map(scan_row).unwrap_or_default();
    if !sql.contains(SQL_CALL_PREFIX) {
        return row;
    }
    with_sql_macro_calls(sql, row, syntax)
}

/// The scan with each outer SQL macro call as one kind-2 reference, replacing those inside it.
fn with_sql_macro_calls(sql: &str, row: ScanRow, syntax: Option<&LexicalSyntax>) -> ScanRow {
    let (references, stop) = row;
    let Ok(calls) = sql_macro_calls(sql, syntax) else {
        return (references, stop);
    };
    if calls.is_empty() {
        return (references, stop);
    }
    let spans: Vec<(usize, usize, String)> = calls
        .iter()
        .map(|call| {
            let start: usize = sql[..call.start].chars().count();
            (
                start,
                start + sql[call.start..call.replaced_end].chars().count(),
                call.name.clone(),
            )
        })
        .collect();
    let mut merged: Vec<ScannedReference> = references
        .into_iter()
        .filter(|reference| !inside_call(reference, &spans))
        .collect();
    merged.extend(
        spans
            .into_iter()
            .map(|(start, end, name)| ScannedReference {
                kind: SQL_MACRO_REFERENCE_KIND,
                name,
                member: None,
                start,
                end,
            }),
    );
    merged.sort_by_key(|reference| reference.start);
    (merged, stop)
}

fn scan_row(scan: DeclarationReferenceScan) -> ScanRow {
    (
        scan.references.into_iter().map(reference_row).collect(),
        scan.stop.map(|stop| match stop {
            DeclarationReferenceStop::UnclosedQuote => 0,
            DeclarationReferenceStop::UnclosedBlockComment => 1,
            DeclarationReferenceStop::Malformed(DeclarationReferenceKind::Enum) => 2,
            DeclarationReferenceStop::Malformed(DeclarationReferenceKind::Constant) => 3,
        }),
    )
}

fn reference_row(reference: DeclarationReference) -> ScannedReference {
    let kind: u8 = match reference.kind {
        DeclarationReferenceKind::Enum => 0,
        DeclarationReferenceKind::Constant => 1,
    };
    ScannedReference {
        kind,
        name: reference.name,
        member: reference.member,
        start: reference.start,
        end: reference.end,
    }
}

/// Whether a declaration reference stands inside one of the SQL macro call spans.
fn inside_call(reference: &ScannedReference, spans: &[(usize, usize, String)]) -> bool {
    spans
        .iter()
        .any(|(start, end, _)| reference.start >= *start && reference.end <= *end)
}
