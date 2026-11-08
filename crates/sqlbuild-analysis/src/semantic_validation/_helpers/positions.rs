//! Native authored positions, including macro-expansion passes.

use crate::semantic_checks::constants::UDF_NAME_PREFIX;
use crate::semantic_validation::_helpers::diagnostics::column_pattern;
use crate::semantic_validation::_helpers::normalization::normalize;
use crate::semantic_validation::models::{BindingPositions, Expansion, PositionInput};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

const POPULAR_TOKEN_THRESHOLD: usize = 200;
static TOKENS: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(r"\w+|[^\w\s]").map_err(|error| error.to_string()));

impl BindingPositions {
    pub fn new(request: PositionInput) -> Result<Self, String> {
        let PositionInput {
            authored,
            query,
            expanded,
            cleaned,
            passes,
        } = request;
        let mapped = normalize(
            &expanded,
            "snowflake",
            (&HashMap::new(), &HashMap::new()),
            UDF_NAME_PREFIX,
        );
        let offsets = match mapped {
            Ok(mapped) if mapped.sql == cleaned => mapped.offsets,
            _ if expanded == cleaned => (0..=expanded.chars().count()).collect(),
            _ => align_offsets(&expanded, &cleaned)?,
        };
        Ok(Self {
            query_start: authored
                .find(&query)
                .map(|offset| authored[..offset].chars().count()),
            lines: line_starts(&authored),
            cleaned_lines: line_starts(&cleaned),
            authored,
            offsets,
            passes,
            words: OnceLock::new(),
        })
    }

    pub fn position(
        &self,
        start: Option<usize>,
        line: Option<usize>,
        column: Option<usize>,
        message: &str,
    ) -> Result<(Option<usize>, Option<usize>), String> {
        if let Some(captures) = column_pattern()?.captures(message) {
            let identifier = captures[1].rsplit('.').next().unwrap_or(&captures[1]);
            if let Some(offset) = self.unique_word_offset(identifier) {
                return Ok(position(&self.lines, offset));
            }
        }
        let Some(query_start) = self.query_start else {
            return Ok((None, None));
        };
        let offset = start.or_else(|| {
            line.zip(column).map(|(line, column)| {
                self.cleaned_lines
                    .get(line.saturating_sub(1))
                    .copied()
                    .unwrap_or(0)
                    + column.saturating_sub(1)
            })
        });
        let Some(offset) = offset else {
            return Ok(position(&self.lines, query_start));
        };
        let mut mapped = self
            .offsets
            .get(offset)
            .copied()
            .unwrap_or_else(|| *self.offsets.last().unwrap_or(&0));
        for pass in self.passes.iter().rev() {
            let mut adjusted = mapped as isize;
            for &Expansion {
                source_start,
                source_end,
                output_start,
                output_end,
            } in pass
            {
                if mapped < output_start {
                    break;
                }
                if mapped < output_end {
                    adjusted = source_start as isize;
                    break;
                }
                adjusted +=
                    (source_end - source_start) as isize - (output_end - output_start) as isize;
            }
            mapped = adjusted.max(0) as usize;
        }
        Ok(position(&self.lines, query_start + mapped))
    }
}

impl BindingPositions {
    /// Char offset of the only whole-word occurrence of `identifier` in the authored SQL.
    pub(crate) fn unique_word_offset(&self, identifier: &str) -> Option<usize> {
        if identifier.is_empty() || !identifier.chars().all(word) {
            let mut occurrences = self
                .authored
                .match_indices(identifier)
                .filter(|(offset, _)| {
                    let before = self.authored[..*offset].chars().next_back();
                    let after = self.authored[*offset + identifier.len()..].chars().next();
                    !before.is_some_and(word) && !after.is_some_and(word)
                });
            let (offset, _) = occurrences.next()?;
            return occurrences
                .next()
                .is_none()
                .then(|| self.authored[..offset].chars().count());
        }
        let (offset, unique) = *self
            .words
            .get_or_init(|| word_occurrences(&self.authored))
            .get(identifier)?;
        unique.then_some(offset)
    }
}

/// Index every maximal word run once so identifier lookups do not rescan the SQL.
fn word_occurrences(sql: &str) -> HashMap<String, (usize, bool)> {
    let mut occurrences: HashMap<String, (usize, bool)> = HashMap::new();
    let mut current = String::new();
    let mut start = 0;
    for (index, character) in sql.chars().chain(std::iter::once(' ')).enumerate() {
        if word(character) {
            if current.is_empty() {
                start = index;
            }
            current.push(character);
        } else if !current.is_empty() {
            occurrences
                .entry(std::mem::take(&mut current))
                .and_modify(|entry| entry.1 = false)
                .or_insert((start, true));
        }
    }
    occurrences
}

fn word(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn line_starts(sql: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        sql.chars()
            .enumerate()
            .filter_map(|(index, ch)| (ch == '\n').then_some(index + 1)),
    );
    starts
}

fn position(lines: &[usize], offset: usize) -> (Option<usize>, Option<usize>) {
    let line = lines.partition_point(|start| *start <= offset).max(1);
    (Some(line), Some(offset - lines[line - 1] + 1))
}

fn align_offsets(original: &str, normalized: &str) -> Result<Vec<usize>, String> {
    let tokens = TOKENS.as_ref().map_err(Clone::clone)?;
    let a: Vec<_> = tokens.find_iter(original).collect();
    let b: Vec<_> = tokens.find_iter(normalized).collect();
    let av: Vec<_> = a
        .iter()
        .map(|token| token.as_str().to_lowercase())
        .collect();
    let bv: Vec<_> = b
        .iter()
        .map(|token| token.as_str().to_lowercase())
        .collect();
    let mut index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (position, token) in bv.iter().enumerate() {
        index.entry(token).or_default().push(position);
    }
    if b.len() >= POPULAR_TOKEN_THRESHOLD {
        index.retain(|_, positions| positions.len() <= b.len() / 100 + 1);
    }
    let mut pending = vec![(0, a.len(), 0, b.len())];
    let mut blocks: Vec<(usize, usize, usize)> = Vec::new();
    while let Some((alo, ahi, blo, bhi)) = pending.pop() {
        let (mut ai, mut bi, mut size) = (alo, blo, 0);
        let mut previous: HashMap<usize, usize> = HashMap::new();
        for (i, token) in av.iter().enumerate().take(ahi).skip(alo) {
            let mut current: HashMap<usize, usize> = HashMap::new();
            for &j in index.get(token.as_str()).into_iter().flatten() {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let length = j
                    .checked_sub(1)
                    .and_then(|j| previous.get(&j))
                    .copied()
                    .unwrap_or(0)
                    + 1;
                current.insert(j, length);
                if length > size {
                    ai = i + 1 - length;
                    bi = j + 1 - length;
                    size = length;
                }
            }
            previous = current;
        }
        while ai > alo && bi > blo && av[ai - 1] == bv[bi - 1] {
            ai -= 1;
            bi -= 1;
            size += 1;
        }
        while ai + size < ahi && bi + size < bhi && av[ai + size] == bv[bi + size] {
            size += 1;
        }
        if size > 0 {
            blocks.push((ai, bi, size));
            if alo < ai && blo < bi {
                pending.push((alo, ai, blo, bi));
            }
            if ai + size < ahi && bi + size < bhi {
                pending.push((ai + size, ahi, bi + size, bhi));
            }
        }
    }
    blocks.sort();
    let ac = byte_char_map(original);
    let bc = byte_char_map(normalized);
    let mut mappings: Vec<Expansion> = Vec::new();
    for (ai, bi, size) in blocks {
        for i in 0..size {
            mappings.push(Expansion {
                source_start: ac[a[ai + i].start()],
                source_end: ac[a[ai + i].end()],
                output_start: bc[b[bi + i].start()],
                output_end: bc[b[bi + i].end()],
            });
        }
    }
    let mut offsets = Vec::with_capacity(normalized.chars().count() + 1);
    let original_len = original.chars().count();
    let mut index = 0;
    for offset in 0..=normalized.chars().count() {
        while index < mappings.len() && offset > mappings[index].output_end {
            index += 1;
        }
        offsets.push(match mappings.get(index) {
            Some(&Expansion {
                source_start: start,
                source_end: end,
                output_start: target,
                ..
            }) => start + offset.saturating_sub(target).min(end - start),
            None => original_len,
        });
    }
    Ok(offsets)
}

fn byte_char_map(sql: &str) -> Vec<usize> {
    let mut map = vec![0; sql.len() + 1];
    for (index, (byte, ch)) in sql.char_indices().enumerate() {
        map[byte..byte + ch.len_utf8()].fill(index);
        map[byte + ch.len_utf8()] = index + 1;
    }
    map
}
