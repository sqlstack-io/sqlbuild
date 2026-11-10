//! Expression macro calls followed through the edits that move them, and their typed markers.

use std::borrow::Cow;

use sqlquack_syntax::expressions::tree::constants::{
    TYPED_EXPRESSION_FUNCTION, TYPED_LIKE_EXPRESSION_FUNCTION,
};
use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::analysis_route::types::NativeDialect;
use crate::compiled_project::models::SplicedSpan;
use crate::sql_macros::_helpers::parsing::body::with_line_comment_closed;
use crate::sql_macros::constants::NULL_KEYWORD;
use crate::sql_macros::models::{TextSplice, TypedCall};

/// `calls` moved `offset` bytes later.
pub(crate) fn shifted(calls: Vec<TypedCall>, offset: usize) -> impl Iterator<Item = TypedCall> {
    calls.into_iter().map(move |call| TypedCall {
        start: call.start + offset,
        end: call.end + offset,
        ..call
    })
}

/// Where byte `position` of the source lands, or `None` inside a replaced range.
fn spliced_position(position: usize, splices: &[TextSplice]) -> Option<usize> {
    let mut moved: isize = 0;
    for splice in splices {
        let (start, end) = splice.source;
        if end <= position {
            moved += (splice.output.1 - splice.output.0) as isize - (end - start) as isize;
        } else if start < position {
            return None;
        } else {
            break;
        }
    }
    let Ok(moved_position) = usize::try_from(position as isize + moved) else {
        return None;
    };
    Some(moved_position)
}

/// `calls` through `splices`, sorted and non-overlapping; a call a splice cuts into is dropped.
pub(crate) fn spliced(calls: Vec<TypedCall>, splices: &[TextSplice]) -> Vec<TypedCall> {
    if splices.is_empty() {
        return calls;
    }
    calls
        .into_iter()
        .filter_map(|call| {
            let start: usize = spliced_position(call.start, splices)?;
            let end: usize = spliced_position(call.end, splices)?;
            Some(TypedCall { start, end, ..call })
        })
        .collect()
}

/// Byte offset of each character offset of `text`, one past the end included.
fn byte_offsets(text: &str) -> Option<Vec<usize>> {
    if text.is_ascii() {
        return None;
    }
    let mut offsets: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
    offsets.push(text.len());
    Some(offsets)
}

/// Character-offset `spans` from `source` to `output` as byte splices.
pub(crate) fn byte_splices(source: &str, output: &str, spans: &[SplicedSpan]) -> Vec<TextSplice> {
    let (source_offsets, output_offsets) = (byte_offsets(source), byte_offsets(output));
    let byte = |offsets: &Option<Vec<usize>>, position: usize| match offsets {
        Some(offsets) => offsets.get(position).copied().unwrap_or(position),
        None => position,
    };
    spans
        .iter()
        .map(|span| TextSplice {
            source: (
                byte(&source_offsets, span.source_start),
                byte(&source_offsets, span.source_end),
            ),
            output: (
                byte(&output_offsets, span.output_start),
                byte(&output_offsets, span.output_end),
            ),
        })
        .collect()
}

/// One marker edit: the text inserted at a byte position, ordered among edits there.
struct MarkerEdit {
    position: usize,
    /// Closing edits come before opening ones at one position.
    opening: bool,
    /// Among openings, the outermost call first; among closings, the innermost first.
    order: (usize, usize),
    text: String,
}

/// `returns` as a SQL string literal.
fn quoted(returns: &str) -> String {
    format!("'{}'", returns.replace('\'', "''"))
}

/// `sql` with each call that declares `returns` wrapped in the typed marker, and each insertion.
pub(crate) fn typed_sql(sql: &str, calls: &[TypedCall]) -> (String, Vec<TextSplice>) {
    let mut edits: Vec<MarkerEdit> = Vec::new();
    for (index, call) in calls.iter().enumerate() {
        let (function, asserted): (&str, String) = match (&call.returns, &call.returns_like) {
            (Some(returns), _) => (TYPED_EXPRESSION_FUNCTION, quoted(returns)),
            (None, Some(like)) => (
                TYPED_LIKE_EXPRESSION_FUNCTION,
                with_line_comment_closed(like, true),
            ),
            (None, None) => continue,
        };
        let valid = |position: usize| position <= sql.len() && sql.is_char_boundary(position);
        if call.start >= call.end || !valid(call.start) || !valid(call.end) {
            continue;
        }
        let inner: &str = &sql[call.start..call.end];
        if is_bare_word(inner) {
            continue;
        }
        let line_break: &str = if with_line_comment_closed(inner, true).len() > inner.len() {
            "\n"
        } else {
            ""
        };
        edits.push(MarkerEdit {
            position: call.start,
            opening: true,
            order: (usize::MAX - call.end, usize::MAX - index),
            text: format!("{function}("),
        });
        edits.push(MarkerEdit {
            position: call.end,
            opening: false,
            order: (usize::MAX - call.start, index),
            text: format!("{line_break}, {asserted})"),
        });
    }
    if edits.is_empty() {
        return (sql.to_owned(), Vec::new());
    }
    edits.sort_by_key(|edit| (edit.position, edit.opening, edit.order));
    let mut typed: String = String::with_capacity(sql.len() + edits.len() * 16);
    let mut insertions: Vec<TextSplice> = Vec::with_capacity(edits.len());
    let mut cursor: usize = 0;
    for edit in edits {
        typed.push_str(&sql[cursor..edit.position]);
        cursor = edit.position;
        insertions.push(TextSplice {
            source: (edit.position, edit.position),
            output: (typed.len(), typed.len() + edit.text.len()),
        });
        typed.push_str(&edit.text);
    }
    typed.push_str(&sql[cursor..]);
    (typed, insertions)
}

/// `calls` without any whose marker changes how `sql` parses; none where `dialect` has no parser.
pub(crate) fn markable_calls<'c>(
    sql: &str,
    calls: &'c [TypedCall],
    dialect: &str,
) -> Cow<'c, [TypedCall]> {
    let marked: usize = calls.iter().filter(|call| is_marked(sql, call)).count();
    if marked == 0 {
        return Cow::Borrowed(calls);
    }
    let AnalysisRoute::Native(parse_dialect) = analysis_route(Some(dialect)) else {
        return Cow::Owned(Vec::new());
    };
    if lowered_markers(&typed_sql(sql, calls).0, parse_dialect) == Some(marked) {
        return Cow::Borrowed(calls);
    }
    let mut kept: Vec<TypedCall> = Vec::with_capacity(calls.len());
    let mut kept_marked: usize = 0;
    for call in calls {
        if !is_marked(sql, call) {
            kept.push(call.clone());
            continue;
        }
        kept.push(call.clone());
        if lowered_markers(&typed_sql(sql, &kept).0, parse_dialect) == Some(kept_marked + 1) {
            kept_marked += 1;
        } else {
            kept.pop();
        }
    }
    Cow::Owned(kept)
}

/// Whether `typed_sql` wraps `call` in a marker.
fn is_marked(sql: &str, call: &TypedCall) -> bool {
    (call.returns.is_some() || call.returns_like.is_some())
        && sql
            .get(call.start..call.end)
            .is_some_and(|text| !is_bare_word(text))
}

/// Whether `text` is one identifier or keyword other than `NULL`, which a marker could misplace.
fn is_bare_word(text: &str) -> bool {
    let word: &str = text.trim();
    if word.eq_ignore_ascii_case(NULL_KEYWORD) {
        return false;
    }
    let mut characters = word.chars();
    characters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && characters.all(|character| character.is_alphanumeric() || character == '_')
}

/// How many typed markers `sql` lowers to expression nodes in `parser`, if it reads `sql` whole.
fn lowered_markers(sql: &str, parser: &dyn NativeDialect) -> Option<usize> {
    let statements: Vec<Expression> = match parser.parse(sql) {
        Ok(Parsed { statements, gaps }) if gaps.is_empty() => statements,
        _ => return None,
    };
    let mut lowered: usize = 0;
    for statement in &statements {
        lowered += statement
            .dfs()
            .filter(|node| is_lowered_marker(node))
            .count();
    }
    Some(lowered)
}

/// Whether `node` is a typed marker lowered to its expression node.
fn is_lowered_marker(node: &Expression) -> bool {
    matches!(node, Expression::Annotated(annotated)
        if annotated.type_assertion.is_some() || annotated.type_like.is_some())
}

/// `text` with each typed marker call, as written in a message, replaced by its first argument.
pub(crate) fn without_marker_calls(text: &str) -> Cow<'_, str> {
    let lowered: String = text.to_ascii_lowercase();
    if !lowered.contains(TYPED_EXPRESSION_FUNCTION) {
        return Cow::Borrowed(text);
    }
    let mut current: String = text.to_owned();
    while let Some(start) = current.to_ascii_lowercase().find(TYPED_EXPRESSION_FUNCTION) {
        let replaced: Option<String> = marker_argument(&current, start)
            .map(|(argument, end)| format!("{}{argument}{}", &current[..start], &current[end..]));
        current = match replaced {
            Some(replaced) => replaced,
            None => format!(
                "{}expression{}",
                &current[..start],
                &current[start + TYPED_EXPRESSION_FUNCTION.len()..]
            ),
        };
    }
    Cow::Owned(current)
}

/// The first argument of the marker call at byte `start` of `text`, and the byte after the call.
fn marker_argument(text: &str, start: usize) -> Option<(String, usize)> {
    let open: usize = start + text[start..].find('(')?;
    let name: &str = &text[start..open];
    if !name.eq_ignore_ascii_case(TYPED_EXPRESSION_FUNCTION)
        && !name.eq_ignore_ascii_case(TYPED_LIKE_EXPRESSION_FUNCTION)
    {
        return None;
    }
    let mut depth: usize = 0;
    let mut quote: Option<char> = None;
    let mut comma: Option<usize> = None;
    for (offset, character) in text[open..].char_indices() {
        let position: usize = open + offset;
        match (quote, character) {
            (Some(open_quote), _) if character == open_quote => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(character),
            (None, '(') => depth += 1,
            (None, ',') if depth == 1 && comma.is_none() => comma = Some(position),
            (None, ')') => {
                depth -= 1;
                if depth == 0 {
                    let argument_end: usize = comma.unwrap_or(position);
                    return Some((text[open + 1..argument_end].trim().to_owned(), position + 1));
                }
            }
            _ => {}
        }
    }
    None
}
