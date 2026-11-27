//! Header string delimiters, prose values and quote failures.

/// How a header string is delimited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderQuote {
    /// `"""..."""`: the text is taken as written, quotes included.
    Triple,
    /// `"..."`.
    Double,
    /// `'...'`.
    Single,
}

/// One prose string of a header: where it is and the folded text it stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProseString {
    pub quote: HeaderQuote,
    /// Character offsets of the opening delimiter and just past the closing one.
    pub start: usize,
    pub end: usize,
    /// The folded text the header stores.
    pub value: String,
}

/// A header quote error located in its file, with the message and help it reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuoteFailure {
    /// The character offset of the offending quote in the header text.
    pub position: usize,
    /// One-based file line and column of the offending quote.
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub help: String,
}

/// The `description` keys of one header text, read forward once however many strings ask.
#[derive(Clone, Debug)]
pub struct DescriptionKeys<'a, T> {
    pub(crate) text: &'a [T],
    /// The offset up to which `open_parens` describes `text`.
    pub(crate) read_to: usize,
    /// Whether each parenthesis still open before `read_to` opens a `meta (...)` value.
    pub(crate) open_parens: Vec<bool>,
    /// How many of `open_parens` open a `meta (...)` value.
    pub(crate) open_meta: usize,
}
