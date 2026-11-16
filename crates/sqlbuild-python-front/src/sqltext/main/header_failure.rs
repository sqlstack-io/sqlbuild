//! A header syntax error's detail, help and file line (when `header_line` is known).

/// A header syntax error's detail, help and file line (when `header_line` is known).
pub fn header_failure(
    error: &str,
    header: &str,
    header_line: Option<usize>,
) -> (String, Option<String>, Option<usize>) {
    let nesting = sqlbuild_sqltext::compiler::main::model_header_errors::header_nesting_failure(
        error,
        header,
        header_line.unwrap_or(1),
    );
    if let Some(quote) = sqlbuild_sqltext::header_strings::main::quote_failure::quote_failure(
        error,
        header,
        (header_line.unwrap_or(1), 1),
    ) {
        return (
            quote.message,
            Some(quote.help),
            header_line.map(|_| quote.line),
        );
    }
    match (nesting, header_line) {
        (Some(nesting), Some(_)) => (nesting.message, Some(nesting.help), Some(nesting.line)),
        (Some(nesting), None) => (error.to_owned(), Some(nesting.help), None),
        (None, _) => (error.to_owned(), None, None),
    }
}
