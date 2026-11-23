//! Replace macro call sites with their rendered SQL and report each substitution span.

use std::borrow::Cow;

use sqlbuild_sqltext::sql_scan::main::bounded_expansions::bounded_expansions;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::macro_calls::_helpers::offsets::byte_offsets;
use crate::macro_calls::models::MacroSpliceSpan;

/// Splice `outputs`, bounded against comments, over ascending code-point `sites`; `None` when they do not fit `sql`.
pub fn splice_macro_calls(
    sql: &str,
    sites: &[(usize, usize)],
    outputs: &[String],
    syntax: Option<&LexicalSyntax>,
) -> Option<(String, Vec<MacroSpliceSpan>)> {
    if sites.len() != outputs.len() {
        return None;
    }
    let boundaries: Vec<usize> = sites
        .iter()
        .flat_map(|(start, end)| [*start, *end])
        .collect();
    if boundaries.windows(2).any(|pair| pair[0] > pair[1]) {
        return None;
    }
    let bytes = byte_offsets(sql, &boundaries)?;
    let expansions: Vec<(usize, usize, &str)> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .zip(outputs)
        .map(|(range, output)| (range[0], range[1], output.as_str()))
        .collect();
    let mut outputs: Vec<Cow<'_, str>> = outputs
        .iter()
        .map(|output| output.as_str().into())
        .collect();
    for bounded in bounded_expansions(sql, &expansions, syntax) {
        outputs[bounded.index] = bounded.text.into();
    }
    let mut rendered =
        String::with_capacity(sql.len() + outputs.iter().map(|output| output.len()).sum::<usize>());
    let mut spans = Vec::with_capacity(sites.len());
    let mut cursor_byte = 0;
    let mut output_chars = 0;
    for ((site, output), range) in sites.iter().zip(&outputs).zip(bytes.as_chunks::<2>().0) {
        let literal = &sql[cursor_byte..range[0]];
        rendered.push_str(literal);
        output_chars += literal.chars().count();
        rendered.push_str(output);
        let output_len = output.chars().count();
        spans.push(MacroSpliceSpan {
            source_start: site.0,
            source_end: site.1,
            output_start: output_chars,
            output_end: output_chars + output_len,
        });
        output_chars += output_len;
        cursor_byte = range[1];
    }
    rendered.push_str(&sql[cursor_byte..]);
    Some((rendered, spans))
}
