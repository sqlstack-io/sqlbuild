//! Where spliced text would open or extend a comment, read with the dialect's own scanner.

use std::sync::LazyLock;

use crate::sql_scan::_helpers::dialect::non_code_end;
use crate::sql_scan::constants::{BLOCK_COMMENT_OPEN, GENERIC_LINE_COMMENT_PREFIX};
use crate::sql_scan::models::{BoundedExpansion, LexicalSyntax, Unclosed};

/// What callers without a dialect scan with: the comments every dialect has.
static GENERIC: LazyLock<LexicalSyntax> = LazyLock::new(|| LexicalSyntax {
    line_comment_prefixes: vec![GENERIC_LINE_COMMENT_PREFIX.to_owned()],
    ..LexicalSyntax::default()
});
const SPACE: &str = " ";
const NEWLINE: &str = "\n";

/// What to put before and after one expansion.
#[derive(Clone, Copy, Default)]
struct Padding {
    before: &'static str,
    after: &'static str,
}

/// The changed expansions, found up to the first unclosed quote or comment; texts that can't form a comment at a boundary are not scanned.
pub(crate) fn bounded(
    sql: &str,
    expansions: &[(usize, usize, &str)],
    syntax: Option<&LexicalSyntax>,
) -> Vec<BoundedExpansion> {
    let syntax: &LexicalSyntax = syntax.unwrap_or(&GENERIC);
    if !may_form_comment(sql, expansions, syntax) {
        return Vec::new();
    }
    let mut walk = Walk {
        syntax,
        padding: vec![Padding::default(); expansions.len()],
        code_before: None,
        previous: None,
        open_comment: None,
    };
    let _unclosed: Result<(), Unclosed> = walk.over(sql, expansions);
    walk.padding
        .iter()
        .zip(expansions)
        .enumerate()
        .filter(|(_, (padding, _))| !(padding.before.is_empty() && padding.after.is_empty()))
        .map(|(index, (padding, (_, _, text)))| BoundedExpansion {
            index,
            text: format!("{}{text}{}", padding.before, padding.after),
            lead: padding.before.len(),
        })
        .collect()
}

/// Whether any expansion holds a line comment or meets a neighbour in a comment opener.
fn may_form_comment(
    sql: &str,
    expansions: &[(usize, usize, &str)],
    syntax: &LexicalSyntax,
) -> bool {
    let mut before: Option<u8> = None;
    let mut at: usize = 0;
    for (start, end, text) in expansions {
        let authored: &str = sql.get(at..*start).unwrap_or_default();
        for piece in [authored, *text] {
            if meets_in_opener(before, piece.as_bytes().first().copied(), syntax) {
                return true;
            }
            before = piece.as_bytes().last().copied().or(before);
        }
        if syntax
            .line_comment_prefixes
            .iter()
            .any(|prefix| text.contains(prefix.as_str()))
        {
            return true;
        }
        at = *end;
    }
    let authored: &str = sql.get(at..).unwrap_or_default();
    meets_in_opener(before, authored.as_bytes().first().copied(), syntax)
}

/// Whether `before` and `after`, side by side, open a comment.
fn meets_in_opener(before: Option<u8>, after: Option<u8>, syntax: &LexicalSyntax) -> bool {
    let (Some(before), Some(after)) = (before, after) else {
        return false;
    };
    let pair: [u8; 2] = [before, after];
    pair == BLOCK_COMMENT_OPEN.as_bytes()
        || syntax
            .line_comment_prefixes
            .iter()
            .any(|prefix| prefix.as_bytes() == pair)
}

/// One pass over the SQL as it will read once spliced.
struct Walk<'a> {
    syntax: &'a LexicalSyntax,
    padding: Vec<Padding>,
    /// The last byte so far, when it is code.
    code_before: Option<u8>,
    /// The last expansion placed in code.
    previous: Option<usize>,
    /// The expansion whose last line comment is still open.
    open_comment: Option<usize>,
}

impl Walk<'_> {
    /// Walk the spliced text; a site inside a quote or comment is read as part of it.
    fn over(&mut self, sql: &str, expansions: &[(usize, usize, &str)]) -> Result<(), Unclosed> {
        let mut at: usize = 0;
        for (index, (start, end, text)) in expansions.iter().enumerate() {
            if *start < at {
                continue;
            }
            at = self.read(sql.as_bytes(), (at, *start), None)?;
            if at != *start {
                continue;
            }
            let _read: usize = self.read(text.as_bytes(), (0, text.len()), Some(index))?;
            self.previous = Some(index);
            at = *end;
        }
        let _read: usize = self.read(sql.as_bytes(), (at, sql.len()), None)?;
        Ok(())
    }

    /// Read one piece of authored or expanded text, returning where its last construct ends.
    fn read(
        &mut self,
        bytes: &[u8],
        (from, to): (usize, usize),
        expansion: Option<usize>,
    ) -> Result<usize, Unclosed> {
        let mut index: usize = from;
        while index < to {
            let byte: u8 = bytes[index];
            if index == from && meets_in_opener(self.code_before, Some(byte), self.syntax) {
                self.separate(expansion);
            }
            let end: Option<usize> = non_code_end(bytes, index, self.syntax)?;
            let line_comment: bool = end.is_some() && self.opens_line_comment(&bytes[index..]);
            self.close_comment(
                &bytes[index..end.unwrap_or(index + 1)],
                end.is_some(),
                line_comment,
            );
            match end {
                None => {
                    self.code_before = Some(byte);
                    index += 1;
                }
                Some(end) => {
                    self.code_before = None;
                    if line_comment
                        && expansion.is_some()
                        && end == bytes.len()
                        && bytes[end - 1] != b'\n'
                    {
                        self.open_comment = expansion;
                    }
                    index = end;
                }
            }
        }
        Ok(index)
    }

    /// A space between two bytes that open a comment, or a newline where a space opens one too.
    fn separate(&mut self, expansion: Option<usize>) {
        let space_opens: bool = meets_in_opener(self.code_before, Some(b' '), self.syntax);
        let separator: &'static str = if space_opens { NEWLINE } else { SPACE };
        match (expansion, self.previous) {
            (Some(current), _) => self.padding[current].before = separator,
            (None, Some(previous)) => self.padding[previous].after = separator,
            (None, None) => {}
        }
    }

    /// End the open comment's line before code; a line break or another line comment ends it already.
    fn close_comment(&mut self, construct: &[u8], non_code: bool, line_comment: bool) {
        let Some(open) = self.open_comment else {
            return;
        };
        if line_comment || construct.first() == Some(&b'\n') {
            self.open_comment = None;
            return;
        }
        let harmless: bool = if non_code {
            construct.starts_with(BLOCK_COMMENT_OPEN.as_bytes()) && !construct.contains(&b'\n')
        } else {
            construct.iter().all(u8::is_ascii_whitespace)
        };
        if !harmless {
            self.padding[open].after = NEWLINE;
            self.open_comment = None;
        }
    }

    fn opens_line_comment(&self, rest: &[u8]) -> bool {
        !rest.starts_with(BLOCK_COMMENT_OPEN.as_bytes())
            && self
                .syntax
                .line_comment_prefixes
                .iter()
                .any(|prefix| rest.starts_with(prefix.as_bytes()))
    }
}
