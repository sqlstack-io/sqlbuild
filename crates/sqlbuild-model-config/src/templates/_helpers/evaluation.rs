//! The Python template evaluator: references, `if`, `eq`, `ne` and `coalesce`.

use crate::templates::_helpers::missing_values::is_missing_value_message;
use crate::templates::_helpers::substitution::rendered;
use crate::templates::errors::TemplateError;
use crate::templates::main::template_error_message::template_error_message;
use crate::templates::models::{
    ContextValue, Expression, Scalar, TemplateFailure, TemplateOptions, TextPart,
};
use crate::templates::types::TemplateHost;

const TRUE_LITERAL: &str = "true";
const FALSE_LITERAL: &str = "false";
const NULL_LITERAL: &str = "null";
const FALSE_VALUES: [&str; 3] = ["", "0", "false"];
const ENVIRONMENT_NAMESPACE: &str = "ENV";
const IF_FUNCTION: &str = "if";
const EQ_FUNCTION: &str = "eq";
const NE_FUNCTION: &str = "ne";
const COALESCE_FUNCTION: &str = "coalesce";
const CONTEXT_NAMESPACE: &str = "CTX";
const CONDITIONAL_ARGUMENTS: &str = "3 arguments";
const COMPARISON_ARGUMENTS: &str = "2 arguments";
const COALESCE_ARGUMENTS: &str = "at least 1 argument";

/// Evaluate one expression to the value Python's resolver returns.
pub(crate) fn evaluate<H: TemplateHost>(
    host: &H,
    expression: &Expression,
    options: TemplateOptions,
) -> Result<H::Value, TemplateFailure> {
    match expression {
        Expression::Text(text) => host.text(text),
        Expression::Interpolated(parts) => {
            let mut joined = String::new();
            for part in parts {
                match part {
                    TextPart::Text(text) => joined.push_str(text),
                    TextPart::Template { body, expression } => {
                        joined.push_str(&rendered(host, body, expression, options)?);
                    }
                }
            }
            host.text(&joined)
        }
        Expression::Reference(reference) => evaluate_reference(host, reference, options),
        Expression::Function { name, arguments } => {
            evaluate_function(host, name, arguments, options)
        }
    }
}

fn evaluate_reference<H: TemplateHost>(
    host: &H,
    reference: &str,
    options: TemplateOptions,
) -> Result<H::Value, TemplateFailure> {
    match reference {
        TRUE_LITERAL => return host.boolean(true),
        FALSE_LITERAL => return host.boolean(false),
        NULL_LITERAL => return Ok(host.null()),
        _ => {}
    }
    let Some((namespace, name)) = reference.split_once(':') else {
        return host.variable(reference)?.ok_or_else(|| {
            TemplateFailure::Missing(TemplateError::UnknownVariable(reference.to_owned()))
        });
    };
    match namespace {
        ENVIRONMENT_NAMESPACE => host.environment(name)?.ok_or_else(|| {
            TemplateFailure::Missing(TemplateError::MissingEnvironment(name.to_owned()))
        }),
        CONTEXT_NAMESPACE if !options.allow_context => {
            if options.preserve_context_tokens {
                host.text(&format!("${{{reference}}}"))
            } else {
                Err(TemplateFailure::Invalid(TemplateError::ContextNotAllowed))
            }
        }
        CONTEXT_NAMESPACE => match host.context(name)? {
            ContextValue::Value(value) => Ok(value),
            ContextValue::Unknown if options.preserve_unknown_context => {
                host.text(&format!("${{{CONTEXT_NAMESPACE}:{name}}}"))
            }
            ContextValue::Unknown => Err(TemplateFailure::Missing(TemplateError::UnknownContext(
                name.to_owned(),
            ))),
            ContextValue::Unavailable => Err(TemplateFailure::Missing(
                TemplateError::UnavailableContext(name.to_owned()),
            )),
        },
        _ => Err(TemplateFailure::Invalid(
            TemplateError::UnsupportedNamespace(namespace.to_owned()),
        )),
    }
}

fn evaluate_function<H: TemplateHost>(
    host: &H,
    name: &str,
    arguments: &[Expression],
    options: TemplateOptions,
) -> Result<H::Value, TemplateFailure> {
    match (name, arguments) {
        (IF_FUNCTION, [condition, when_true, when_false]) => {
            let condition = evaluate(host, condition, options)?;
            if truthiness(host, &condition)? {
                evaluate(host, when_true, options)
            } else {
                evaluate(host, when_false, options)
            }
        }
        (IF_FUNCTION, _) => Err(argument_count(IF_FUNCTION, CONDITIONAL_ARGUMENTS)),
        (EQ_FUNCTION | NE_FUNCTION, [left, right]) => {
            let left = evaluate(host, left, options)?;
            let right = evaluate(host, right, options)?;
            let equal = comparison_text(host, &left)? == comparison_text(host, &right)?;
            host.boolean(equal == (name == EQ_FUNCTION))
        }
        (EQ_FUNCTION, _) => Err(argument_count(EQ_FUNCTION, COMPARISON_ARGUMENTS)),
        (NE_FUNCTION, _) => Err(argument_count(NE_FUNCTION, COMPARISON_ARGUMENTS)),
        (COALESCE_FUNCTION, [.., last]) => {
            for argument in arguments {
                match evaluate(host, argument, options) {
                    Ok(value) if truthiness(host, &value)? => return Ok(value),
                    Ok(_) => {}
                    Err(TemplateFailure::Missing(error) | TemplateFailure::Invalid(error))
                        if is_missing_value_message(&template_error_message(
                            &error,
                            host.label(),
                        )) => {}
                    Err(failure) => return Err(failure),
                }
            }
            evaluate(host, last, options)
        }
        (COALESCE_FUNCTION, []) => Err(argument_count(COALESCE_FUNCTION, COALESCE_ARGUMENTS)),
        _ => Err(TemplateFailure::Invalid(
            TemplateError::UnsupportedFunction(name.to_owned()),
        )),
    }
}

fn argument_count(function: &'static str, expected: &'static str) -> TemplateFailure {
    TemplateFailure::Invalid(TemplateError::ArgumentCount { function, expected })
}

fn comparison_text<H: TemplateHost>(host: &H, value: &H::Value) -> Result<String, TemplateFailure> {
    match host.scalar(value)? {
        Scalar::Null => Ok(String::new()),
        Scalar::Bool(flag) => Ok(if flag { TRUE_LITERAL } else { FALSE_LITERAL }.to_owned()),
        Scalar::Text(text) => Ok(text),
    }
}

fn truthiness<H: TemplateHost>(host: &H, value: &H::Value) -> Result<bool, TemplateFailure> {
    match host.scalar(value)? {
        Scalar::Null => Ok(false),
        Scalar::Bool(flag) => Ok(flag),
        Scalar::Text(text) => {
            let normalized = str::trim(&text).to_lowercase();
            Ok(!FALSE_VALUES.contains(&normalized.as_str()))
        }
    }
}
