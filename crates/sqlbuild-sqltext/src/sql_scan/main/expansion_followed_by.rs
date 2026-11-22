//! `expansion_followed_by`: an expansion as it stands before text wrapped around it.

use crate::sql_scan::_helpers::boundaries::bounded;
use crate::sql_scan::models::LexicalSyntax;

/// `text`, bounded so that `next` can be written straight after it.
#[must_use]
pub fn expansion_followed_by(text: &str, next: &str, syntax: Option<&LexicalSyntax>) -> String {
    bounded(next, &[(0, 0, text)], syntax)
        .pop()
        .map_or_else(|| text.to_owned(), |expansion| expansion.text)
}
