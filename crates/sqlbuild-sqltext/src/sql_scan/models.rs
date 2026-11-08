//! Lexical scanning outcomes and dialect-dependent quoting policy.

/// The SQL construct a scan reached the end of input inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unclosed {
    BlockComment,
    Quote,
    Parenthesis,
}

/// Dialect-dependent quoting rules; comments and doubled-quote escapes are fixed for every policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotePolicy {
    /// Backtick-delimited identifiers are quoted text.
    pub backtick_identifiers: bool,
    /// A backslash escapes the next byte inside single-quoted strings.
    pub single_quote_backslash_escapes: bool,
    /// A backslash escapes the next byte inside double-quoted text.
    pub double_quote_backslash_escapes: bool,
    /// `$$` and `$tag$` delimit opaque string literals.
    pub dollar_quotes: bool,
}

impl QuotePolicy {
    /// Compiler policy, mirroring Python `sql_analysis`: backticks and dollar quotes are text.
    pub const COMPILER: Self = Self {
        backtick_identifiers: true,
        single_quote_backslash_escapes: false,
        double_quote_backslash_escapes: false,
        dollar_quotes: true,
    };

    /// SQL lint policy: backslashes escape, backticks per dialect, dollar quotes as in the compiler.
    pub const SQL_LINT: Self = Self {
        backtick_identifiers: false,
        single_quote_backslash_escapes: true,
        double_quote_backslash_escapes: true,
        dollar_quotes: true,
    };

    /// Return this policy with backtick-delimited identifiers enabled or disabled.
    pub const fn with_backtick_identifiers(self, backtick_identifiers: bool) -> Self {
        Self {
            backtick_identifiers,
            ..self
        }
    }

    /// Return whether `byte` opens quoted text under this policy.
    pub fn is_quote(self, byte: u8) -> bool {
        match byte {
            b'\'' | b'"' => true,
            b'`' => self.backtick_identifiers,
            _ => false,
        }
    }

    /// Return whether a backslash escapes the next byte inside text opened by `quote`.
    pub(crate) fn backslash_escapes(self, quote: u8) -> bool {
        match quote {
            b'\'' => self.single_quote_backslash_escapes,
            b'"' => self.double_quote_backslash_escapes,
            _ => false,
        }
    }
}

/// One adapter's rules for where quoted text and comments end, mirroring Python `SqlLexicalSyntax`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalSyntax {
    /// Quote characters inside which a backslash escapes the next byte.
    pub backslash_escape_quotes: Vec<String>,
    /// `E'...'` strings honour backslash escapes.
    pub escape_string_prefix: bool,
    /// `r'...'` strings disable backslash escapes.
    pub raw_string_prefix: bool,
    /// `'''...'''` and `"""..."""` delimit strings.
    pub triple_quoted_strings: bool,
    /// Block comments nest.
    pub nested_block_comments: bool,
    /// Prefixes that start a line comment, such as `--`, `#` or `//`.
    pub line_comment_prefixes: Vec<String>,
    /// `[...]` quotes an identifier, `]]` standing for one `]`, as in T-SQL.
    #[serde(default)]
    pub bracket_identifiers: bool,
}

impl LexicalSyntax {
    /// Return whether a backslash escapes the next byte inside text opened by `quote`.
    pub(crate) fn backslash_escapes(&self, quote: u8) -> bool {
        self.backslash_escape_quotes
            .iter()
            .any(|value| value.as_bytes() == [quote])
    }
}
