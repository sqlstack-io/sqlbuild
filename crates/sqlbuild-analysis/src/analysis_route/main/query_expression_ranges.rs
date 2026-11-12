//! Expression ranges of a model's native analysis, moved back into the query SQL it compiled.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::semantic_validation::main::normalize_native_filled::normalize_native_filled;
use crate::sql_macros::models::TextSplice;

/// Each cleaned-SQL byte range as a range of the SQL `passes` rewrote into `analysis_sql`.
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
        normalize_native_filled(analysis_sql, dialect, module.stub_prefix(), placeholders)
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
        if !copied_boundary(&offsets, character) {
            return None;
        }
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

/// Whether cleaned boundary `character` lies between source-adjacent characters, so maps exactly.
fn copied_boundary(offsets: &[usize], character: usize) -> bool {
    let Some(previous) = character.checked_sub(1) else {
        return offsets.first() == Some(&0);
    };
    offsets
        .get(previous)
        .zip(offsets.get(character))
        .is_some_and(|(before, after)| before + 1 == *after)
}

/// The source byte of output `byte`; splice edges map to source edges, inner bytes to `None`.
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
