//! Expression ranges of a model's native analysis, moved back into the query SQL it compiled.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped;
use crate::sql_macros::models::TextSplice;

/// Each byte range of `analysis_sql`'s cleaned native analysis SQL as the byte range of the query
/// SQL that `passes` rewrote, in order, into `analysis_sql`; `None` where there was none, where an
/// end falls inside rewritten text, or where the dialect has no native analysis.
#[must_use]
pub fn query_expression_ranges(
    dialect: &str,
    (analysis_sql, placeholders): (&str, &[(String, String)]),
    passes: &[Vec<TextSplice>],
    ranges: &[Option<(usize, usize)>],
) -> Vec<Option<(usize, usize)>> {
    let AnalysisRoute::Native(module) = analysis_route(Some(dialect)) else {
        return vec![None; ranges.len()];
    };
    let Ok((cleaned, offsets)) =
        normalize_native_sql_mapped(analysis_sql, dialect, module.stub_prefix(), placeholders)
    else {
        return vec![None; ranges.len()];
    };
    let analysis_bytes: Vec<usize> = analysis_sql
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(analysis_sql.len()))
        .collect();
    let mut characters: Vec<Option<usize>> = vec![None; cleaned.len() + 1];
    for (character, (byte, _)) in cleaned.char_indices().enumerate() {
        characters[byte] = Some(character);
    }
    characters[cleaned.len()] = Some(cleaned.chars().count());
    let query_byte = |cleaned_byte: usize| -> Option<usize> {
        let character: usize = (*characters.get(cleaned_byte)?)?;
        let byte: usize = *analysis_bytes.get(*offsets.get(character)?)?;
        passes
            .iter()
            .rev()
            .try_fold(byte, |byte, splices| spliced_back(byte, splices))
    };
    ranges
        .iter()
        .map(|range| {
            let (start, end) = (*range)?;
            let (start, end) = (query_byte(start)?, query_byte(end)?);
            (start <= end).then_some((start, end))
        })
        .collect()
}

/// The source byte of output byte `byte` of a pass whose `splices` are in text order; a byte at
/// a splice's edge maps to the same edge of its source, and one inside rewritten text has none.
fn spliced_back(byte: usize, splices: &[TextSplice]) -> Option<usize> {
    let (mut source_end, mut output_end): (usize, usize) = (0, 0);
    for splice in splices {
        if byte < splice.output.0 {
            break;
        }
        if byte == splice.output.0 {
            return Some(splice.source.0);
        }
        if byte < splice.output.1 {
            return None;
        }
        if byte == splice.output.1 {
            return Some(splice.source.1);
        }
        (source_end, output_end) = (splice.source.1, splice.output.1);
    }
    Some(byte - output_end + source_end)
}
