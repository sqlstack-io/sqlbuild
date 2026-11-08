//! A type as written, respelt with the names SQLBuild's type fallback compares.

use crate::analysis_route::constants::{FALLBACK_TIMESTAMP_WORD, MAX_TYPE_NAME_WORDS};
use crate::analysis_route::models::TypeSpelling;

/// `spelling` with each type name renamed by `rules`, or `None` when it is not one type.
pub(crate) fn respelt(spelling: &str, rules: &TypeSpelling) -> Option<String> {
    let mut reader = Reader {
        source: spelling,
        text: spelling.as_bytes(),
        position: 0,
        rules,
    };
    let respelt: String = reader.data_type(true)?;
    reader.blank();
    (reader.position == reader.text.len()).then_some(respelt)
}

struct Reader<'a> {
    source: &'a str,
    text: &'a [u8],
    position: usize,
    rules: &'a TypeSpelling,
}

impl Reader<'_> {
    /// One type, renamed, with its parameters and array suffixes; an array's element is nested.
    fn data_type(&mut self, top_level: bool) -> Option<String> {
        let start: usize = self.position;
        let mut respelt: String = self.element(false)?;
        if top_level && self.peek_after_blank() != Some(b'[') {
            self.position = start;
            respelt = self.element(true)?;
        }
        let parameterised: usize = respelt.len();
        while self.peek_after_blank() == Some(b'[') {
            if !self.rules.arrays {
                return None;
            }
            self.blank();
            let suffix: usize = self.position;
            self.position += 1;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
            }
            if self.peek() != Some(b']') {
                return None;
            }
            self.position += 1;
            respelt.push_str(self.source.get(suffix..self.position)?);
        }
        let unsplit: bool = respelt.len() > parameterised || respelt.matches(')').count() > 1;
        let misread: bool = unsplit && respelt.to_uppercase().contains(FALLBACK_TIMESTAMP_WORD);
        (!(top_level && misread)).then_some(respelt)
    }

    /// One type without array suffixes, renamed.
    fn element(&mut self, top_level: bool) -> Option<String> {
        let (written, renamed) = self.name(top_level)?;
        let mut respelt: String = renamed.clone();
        self.blank();
        if self.peek() != Some(b'(') && self.rules.bare_kept.contains(&written.as_str()) {
            respelt.clone_from(&written);
        }
        if self.peek() == Some(b'(') {
            if self.rules.verbatim.contains(&renamed.as_str()) {
                respelt.push_str(&self.balanced()?);
            } else {
                let fielded: bool = self.rules.fielded.contains(&renamed.as_str());
                let arguments: Vec<String> = self.arguments(fielded)?;
                if top_level && self.rules.sizeless_whole.contains(&written.as_str()) {
                    return None;
                }
                if !self.rules.sizeless.contains(&written.as_str()) {
                    respelt = format!("{respelt}({})", arguments.join(", "));
                }
            }
        }
        Some(respelt)
    }

    /// The longest type name the rules rename, or one word as written, upper-cased.
    fn name(&mut self, top_level: bool) -> Option<(String, String)> {
        self.blank();
        let mut words: Vec<(String, usize)> = Vec::new();
        while words.len() < MAX_TYPE_NAME_WORDS {
            self.blank_if(!words.is_empty());
            let word_start: usize = self.position;
            self.position = self.word_end();
            if self.position == word_start {
                break;
            }
            let word: String = self.source.get(word_start..self.position)?.to_uppercase();
            words.push((word, self.position));
        }
        let (first, first_end) = words.first().cloned()?;
        for length in (1..=words.len()).rev() {
            let phrase: String = words[..length]
                .iter()
                .map(|(word, _)| word.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let nested: &[(&str, &str)] = if top_level {
                &[]
            } else {
                self.rules.nested_aliases
            };
            if let Some((_, renamed)) = self
                .rules
                .aliases
                .iter()
                .chain(nested)
                .find(|(from, _)| *from == phrase)
            {
                self.position = words[length - 1].1;
                return Some((phrase, (*renamed).to_owned()));
            }
        }
        self.position = first_end;
        Some((first.clone(), first))
    }

    /// A parenthesised argument list: field and type pairs, or types and literals.
    fn arguments(&mut self, fielded: bool) -> Option<Vec<String>> {
        self.position += 1;
        let mut arguments: Vec<String> = Vec::new();
        loop {
            self.blank();
            let argument: String = if fielded {
                let field: String = self.field()?;
                format!("{field} {}", self.data_type(false)?)
            } else if self
                .peek()
                .is_some_and(|byte| byte.is_ascii_digit() || byte == b'\'')
            {
                self.literal()?
            } else {
                self.data_type(false)?
            };
            arguments.push(argument);
            self.blank();
            match self.peek()? {
                b',' => self.position += 1,
                b')' => {
                    self.position += 1;
                    return Some(arguments);
                }
                _ => return None,
            }
        }
    }

    /// A field name as written; a bare one outside ASCII is quoted, as Polyglot quotes it.
    fn field(&mut self) -> Option<String> {
        let start: usize = self.position;
        if self.peek() == Some(b'"') {
            self.position += 1;
            while self.peek()? != b'"' {
                self.position += 1;
            }
            self.position += 1;
        } else {
            self.position = self.word_end();
        }
        let field: &str = self.source.get(start..self.position)?;
        match (field.is_empty(), field.starts_with('"') || field.is_ascii()) {
            (true, _) => None,
            (false, true) => Some(field.to_owned()),
            (false, false) => Some(format!("\"{field}\"")),
        }
    }

    /// A number or a quoted string, as written.
    fn literal(&mut self) -> Option<String> {
        let start: usize = self.position;
        if self.peek() == Some(b'\'') {
            self.position += 1;
            while self.peek()? != b'\'' {
                self.position += 1;
            }
            self.position += 1;
        } else {
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
            }
        }
        self.source.get(start..self.position).map(str::to_owned)
    }

    /// A parenthesised span copied as written, which the fallback reads as text.
    fn balanced(&mut self) -> Option<String> {
        let start: usize = self.position;
        let mut depth: usize = 0;
        loop {
            match self.peek()? {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            self.position += 1;
            if depth == 0 {
                break;
            }
        }
        self.source.get(start..self.position).map(str::to_owned)
    }

    /// Where the word at the position ends: letters and digits in any script, and underscores.
    fn word_end(&self) -> usize {
        let rest: &str = self.source.get(self.position..).unwrap_or_default();
        self.position
            + rest
                .find(|character: char| !(character.is_alphanumeric() || character == '_'))
                .unwrap_or(rest.len())
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.position).copied()
    }

    fn peek_after_blank(&self) -> Option<u8> {
        self.text[self.position..]
            .iter()
            .copied()
            .find(|byte| !byte.is_ascii_whitespace())
    }

    fn blank(&mut self) {
        self.blank_if(true);
    }

    fn blank_if(&mut self, skip: bool) {
        while skip && self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }
}
