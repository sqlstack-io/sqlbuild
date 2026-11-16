//! Substitute each `${...}` template in a string by its rendered value.

use crate::templates::_helpers::evaluation::evaluate;
use crate::templates::_helpers::expressions::parse_expression;
use crate::templates::models::{Expression, TemplateFailure, TemplateOptions};
use crate::templates::types::TemplateHost;

/// `text` with every template at `spans` replaced by its value rendered as text.
pub(crate) fn substituted<H: TemplateHost>(
    host: &H,
    text: &str,
    spans: &[(usize, usize)],
    options: TemplateOptions,
) -> Result<String, TemplateFailure> {
    let mut expanded = String::with_capacity(text.len());
    let mut copied = 0;
    for &(start, end) in spans {
        expanded.push_str(&text[copied..start]);
        let body = &text[start + 2..end - 1];
        expanded.push_str(&rendered(host, body, &parse_expression(body)?, options)?);
        copied = end;
    }
    expanded.push_str(&text[copied..]);
    Ok(expanded)
}

/// The value of the template `body`, parsed as `expression`, rendered as text.
pub(crate) fn rendered<H: TemplateHost>(
    host: &H,
    body: &str,
    expression: &Expression,
    options: TemplateOptions,
) -> Result<String, TemplateFailure> {
    let value = evaluate(host, expression, options)?;
    host.render(&value, &format!("{} variable '{body}'", host.label()))
}
