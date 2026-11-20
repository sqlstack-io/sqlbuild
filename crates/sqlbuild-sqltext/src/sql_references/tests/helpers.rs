//! Lexical syntaxes and expected references for reference extraction tests.

use crate::sql_references::models::{
    InvalidReferenceCall, ReferenceExtraction, ReferenceScan, ReferenceScanFailure, SqlReference,
};
use crate::sql_scan::models::LexicalSyntax;

pub(crate) fn generic() -> LexicalSyntax {
    LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    }
}

pub(crate) fn backslash_hash_comments() -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_string(), "\"".to_string()],
        triple_quoted_strings: true,
        line_comment_prefixes: vec!["--".to_string(), "#".to_string()],
        ..LexicalSyntax::default()
    }
}

pub(crate) fn reference(kind: &'static str, name: &str) -> SqlReference {
    SqlReference {
        kind,
        name: name.to_string(),
        package: None,
        call_argument_count: None,
        call_partitioned: false,
    }
}

pub(crate) fn table_function(name: &str, call_argument_count: usize) -> SqlReference {
    SqlReference {
        call_argument_count: Some(call_argument_count),
        ..reference("table_fn", name)
    }
}

pub(crate) fn partitioned_table_function(name: &str, call_argument_count: usize) -> SqlReference {
    SqlReference {
        call_partitioned: true,
        ..table_function(name, call_argument_count)
    }
}

pub(crate) fn dbt_reference(package: &str, name: &str) -> SqlReference {
    SqlReference {
        package: Some(package.to_string()),
        ..reference("dbt_ref", name)
    }
}

pub(crate) fn extracted(references: Vec<SqlReference>) -> ReferenceExtraction {
    rejected(references, Vec::new())
}

pub(crate) fn rejected(
    references: Vec<SqlReference>,
    invalid_calls: Vec<InvalidReferenceCall>,
) -> ReferenceExtraction {
    ReferenceExtraction::Extracted(ReferenceScan {
        references,
        invalid_calls,
    })
}

pub(crate) fn invalid_call(
    kind: &'static str,
    call: &str,
    start: usize,
    message: &str,
    help: &str,
    corrected_call: &str,
) -> InvalidReferenceCall {
    InvalidReferenceCall {
        kind,
        call: call.to_string(),
        start,
        message: message.to_string(),
        help: help.to_string(),
        corrected_call: corrected_call.to_string(),
    }
}

pub(crate) fn failed(message: &str, start: usize) -> ReferenceExtraction {
    ReferenceExtraction::Failed(ReferenceScanFailure {
        message: message.to_string(),
        start,
    })
}
