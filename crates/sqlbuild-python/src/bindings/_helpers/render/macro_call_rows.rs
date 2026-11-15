//! Python rows of macro call sites and splice spans.

use sqlbuild_analysis::semantic_validation::models::Expansion;
use sqlbuild_python_base::positional_record;
use sqlbuild_render::macro_calls::models::{MacroCallSite, MacroSpliceSpan};
use sqlbuild_sqltext::compiler::models::CharSpan;

positional_record! {
    into;
    /// One call site: start, arguments start, end, name, tree names if scanned and whether typed reference text appears.
    pub(crate) struct SiteRow {
        start: usize,
        args_start: usize,
        end: usize,
        name: String,
        tree_names: Option<Vec<String>>,
        typed_reference_text: bool,
    }
}
/// One substitution span: source start and end, output start and end.
pub(crate) type SpanRow = CharSpan;

positional_record! {
    into;
    /// A span as the tuple Python reads: source start and end, output start and end.
    pub(crate) struct SpanTuple {
        source_start: usize,
        source_end: usize,
        output_start: usize,
        output_end: usize,
    }
}

/// A span as the tuple Python reads.
pub(crate) fn span_tuple(span: &SpanRow) -> SpanTuple {
    SpanTuple {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}

/// A span as the expansion semantic validation maps offsets through.
pub(crate) fn span_expansion(span: SpanRow) -> Expansion {
    Expansion {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}

pub(crate) fn site_row(site: MacroCallSite) -> SiteRow {
    SiteRow {
        start: site.start,
        args_start: site.args_start,
        end: site.end,
        name: site.name,
        tree_names: site.tree_names,
        typed_reference_text: site.typed_reference_text,
    }
}

pub(crate) fn span_row(span: MacroSpliceSpan) -> SpanRow {
    SpanRow {
        source_start: span.source_start,
        source_end: span.source_end,
        output_start: span.output_start,
        output_end: span.output_end,
    }
}
