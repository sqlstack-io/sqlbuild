//! Public compiler entry point for scanning `@enum`/`@const` references.

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::compiler::_helpers::declaration_references::scanner::scan_references;
use crate::compiler::models::DeclarationReferenceScan;
use crate::sql_scan::models::LexicalSyntax;

/// Each SQL string's references and stop; `syntax` decides where quoted text and comments end.
#[must_use]
pub fn scan_declaration_references(
    sqls: &[String],
    skip_malformed: bool,
    syntax: Option<&LexicalSyntax>,
) -> Vec<DeclarationReferenceScan> {
    sqls.par_iter()
        .map(|sql| scan_references(sql, skip_malformed, syntax))
        .collect()
}
