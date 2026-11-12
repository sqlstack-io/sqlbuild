//! Expression ranges of a model's native analysis, moved back into the query SQL it compiled.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped;

/// Each byte range of `query_sql`'s cleaned native analysis SQL as the byte range of `query_sql`
/// it came from; `None` where there was none or the dialect has no native analysis.
#[must_use]
pub fn query_expression_ranges(
    dialect: &str,
    (query_sql, placeholders): (&str, &[(String, String)]),
    ranges: &[Option<(usize, usize)>],
) -> Vec<Option<(usize, usize)>> {
    let AnalysisRoute::Native(module) = analysis_route(Some(dialect)) else {
        return vec![None; ranges.len()];
    };
    let Ok((cleaned, offsets)) =
        normalize_native_sql_mapped(query_sql, dialect, module.stub_prefix(), placeholders)
    else {
        return vec![None; ranges.len()];
    };
    let query_bytes: Vec<usize> = query_sql
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(query_sql.len()))
        .collect();
    let mut characters: Vec<Option<usize>> = vec![None; cleaned.len() + 1];
    for (character, (byte, _)) in cleaned.char_indices().enumerate() {
        characters[byte] = Some(character);
    }
    characters[cleaned.len()] = Some(cleaned.chars().count());
    let query_byte = |cleaned_byte: usize| -> Option<usize> {
        let character: usize = (*characters.get(cleaned_byte)?)?;
        query_bytes.get(*offsets.get(character)?).copied()
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
