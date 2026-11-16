use std::cell::RefCell;

use crate::templates::_helpers::pattern::counted_template_spans;
use crate::templates::errors::TemplateError;
use crate::templates::main::expand_template_string::expand_template_string;
use crate::templates::models::{StringExpansion, TemplateFailure, TemplateOptions};
use crate::templates::tests::helpers::{text, value};
use crate::templates::tests::test_types::{
    TemplateScanTestCase, TemplateTestCase, TestHost, Value,
};

#[test]
fn given_template_strings_when_expanding_then_values_and_reads_match_python() {
    let test_cases = [
        TemplateTestCase {
            description: "a whole-string variable keeps its value",
            text: "${flag}",
            expected: value(Value::Bool(true)),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "embedded templates render as text",
            text: "orders_${env}_${flag}_${null}",
            expected: text("orders_prod_true_"),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "coalesce skips a missing environment variable and re-reads the last",
            text: "${coalesce(ENV:MISSING, ENV:SCHEMA_NAME, 'fallback')}",
            expected: value(Value::Text("analytics".to_owned())),
            expected_reads: &["ENV:MISSING", "ENV:SCHEMA_NAME"],
        },
        TemplateTestCase {
            description: "coalesce returns the last argument when none is truthy",
            text: "${coalesce(ENV:MISSING, ' False ', ENV:EMPTY)}",
            expected: value(Value::Text(String::new())),
            expected_reads: &["ENV:MISSING", "ENV:EMPTY", "ENV:EMPTY"],
        },
        TemplateTestCase {
            description: "if, eq and ne compare rendered text",
            text: "${if(eq(CTX:run.target, 'prod'), 'p', ne(flag, true))}",
            expected: value(Value::Text("p".to_owned())),
            expected_reads: &["CTX:run.target"],
        },
        TemplateTestCase {
            description: "escapes and nested quotes inside strings",
            text: r#"${"a\"b)c"}"#,
            expected: value(Value::Text("a\"b)c".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unknown context key is preserved",
            text: "${CTX:model.alias}",
            expected: value(Value::Text("${CTX:model.alias}".to_owned())),
            expected_reads: &["CTX:model.alias"],
        },
        TemplateTestCase {
            description: "a string without a complete template is unchanged",
            text: "${} and ${a{b}",
            expected: Ok(StringExpansion::Unchanged),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a context key without a value is missing",
            text: "${CTX:model.schema}",
            expected: Err(TemplateFailure::Missing(TemplateError::UnavailableContext(
                "model.schema".to_owned(),
            ))),
            expected_reads: &["CTX:model.schema"],
        },
        TemplateTestCase {
            description: "a missing variable after a coalesce names itself",
            text: "${coalesce(missing, '')}_${ENV:UNSET}",
            expected: Err(TemplateFailure::Missing(TemplateError::MissingEnvironment(
                "UNSET".to_owned(),
            ))),
            expected_reads: &["ENV:UNSET"],
        },
        TemplateTestCase {
            description: "an unexpected token reports its unquoted value and position",
            text: "${coalesce(a,,b)}",
            expected: Err(TemplateFailure::Invalid(TemplateError::UnexpectedToken {
                token: ",".to_owned(),
                position: 11,
            })),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a trailing token after the expression",
            text: "${env 'x y'}",
            expected: Err(TemplateFailure::Invalid(TemplateError::UnexpectedToken {
                token: "x y".to_owned(),
                position: 4,
            })),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unclosed call expects its parenthesis at the end",
            text: "${if(flag, env",
            expected: Ok(StringExpansion::Unchanged),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unclosed call inside a template expects its parenthesis",
            text: "${eq(flag env)}",
            expected: Err(TemplateFailure::Invalid(TemplateError::ExpectedSymbol {
                symbol: ')',
                position: 8,
            })),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unterminated string reports where it opens",
            text: "${'abc}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnterminatedString {
                    quote: "single",
                    position: 0,
                },
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an escape at the end of the expression",
            text: "${\"ab\\}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnterminatedEscape { position: 3 },
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "argument counts are checked before arguments are evaluated",
            text: "${ne(missing)}",
            expected: Err(TemplateFailure::Invalid(TemplateError::ArgumentCount {
                function: "ne",
                expected: "2 arguments",
            })),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unsupported function is invalid",
            text: "${upper(env)}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnsupportedFunction("upper".to_owned()),
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unsupported namespace is invalid",
            text: "${VAR:env}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnsupportedNamespace("VAR".to_owned()),
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a mapping cannot be interpolated into text",
            text: "x_${opaque}",
            expected: Err(TemplateFailure::Invalid(TemplateError::Message(
                "model config variable 'opaque' is an object".to_owned(),
            ))),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "non-ASCII Python whitespace separates tokens",
            text: "${coalesce(\u{a0}env)}",
            expected: value(Value::Text("prod".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a mapping is truthy by its text",
            text: "${if(opaque, 'truthy', 'falsy')}",
            expected: value(Value::Text("truthy".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "non-ASCII text is truthy unless it strips and lowers to a false value",
            text: "${coalesce('\u{3000}FALSE\u{3000}', 'Ünset')}",
            expected: value(Value::Text("Ünset".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "coalesce stops at an error that is not a missing value",
            text: "${coalesce(other:thing, env)}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnsupportedNamespace("other".to_owned()),
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "Python separators are whitespace between tokens",
            text: "${coalesce(\u{3000}env\t)}",
            expected: value(Value::Text("prod".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a quoted argument interpolates templates, so coalesce builds a fallback",
            text: "${coalesce(ENV:MISSING, 'dev_${ENV:SCHEMA_NAME}')}",
            expected: value(Value::Text("dev_analytics".to_owned())),
            expected_reads: &["ENV:MISSING", "ENV:SCHEMA_NAME"],
        },
        TemplateTestCase {
            description: "a set first argument wins before the quoted fallback is read",
            text: "${coalesce(ENV:SCHEMA_NAME, 'dev_${ENV:MISSING}')}",
            expected: value(Value::Text("analytics".to_owned())),
            expected_reads: &["ENV:SCHEMA_NAME"],
        },
        TemplateTestCase {
            description: "a missing variable inside the last quoted argument names itself",
            text: "${coalesce(ENV:MISSING, 'dev_${ENV:UNSET}')}",
            expected: Err(TemplateFailure::Missing(TemplateError::MissingEnvironment(
                "UNSET".to_owned(),
            ))),
            expected_reads: &["ENV:MISSING", "ENV:UNSET", "ENV:UNSET"],
        },
        TemplateTestCase {
            description: "nested templates in if arguments may reuse the outer quote",
            text: "${if(eq(env, 'prod'), 'p_${coalesce(ENV:MISSING, 'x')}', 'n')}",
            expected: value(Value::Text("p_x".to_owned())),
            expected_reads: &["ENV:MISSING"],
        },
        TemplateTestCase {
            description: "an interpolated argument inside text renders into it",
            text: "schema_${coalesce(ENV:EMPTY, 'dev_${env}_${flag}')}_x",
            expected: text("schema_dev_prod_true_x"),
            expected_reads: &["ENV:EMPTY"],
        },
        TemplateTestCase {
            description: "a backslash keeps a literal template in a quoted argument",
            text: r"${coalesce('\${ENV:SCHEMA_NAME}')}",
            expected: value(Value::Text("${ENV:SCHEMA_NAME}".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an unclosed template in a quoted argument keeps the outer one from closing",
            text: "${coalesce('a${b')}",
            expected: Err(TemplateFailure::Invalid(
                TemplateError::UnterminatedString {
                    quote: "single",
                    position: 1,
                },
            )),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "templates nested more than eight deep do not close",
            text: "${coalesce('${coalesce('${coalesce('${coalesce('${coalesce('${coalesce('${coalesce('${coalesce('${coalesce('${env}')}')}')}')}')}')}')}')}')}",
            expected: text("${coalesce('prod')}"),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "a brace inside a quoted argument is text",
            text: "${coalesce('a}b')}",
            expected: value(Value::Text("a}b".to_owned())),
            expected_reads: &[],
        },
        TemplateTestCase {
            description: "an object cannot be interpolated into a quoted argument",
            text: "${coalesce('x_${opaque}')}",
            expected: Err(TemplateFailure::Invalid(TemplateError::Message(
                "model config variable 'opaque' is an object".to_owned(),
            ))),
            expected_reads: &[],
        },
    ];

    for test_case in test_cases {
        let host = TestHost {
            variables: vec![
                ("flag", Value::Bool(true)),
                ("env", Value::Text("prod".to_owned())),
                ("opaque", Value::Opaque),
            ],
            environment: vec![("SCHEMA_NAME", "analytics"), ("EMPTY", "")],
            context: vec![("run.target", Some("prod")), ("model.schema", None)],
            reads: RefCell::new(Vec::new()),
        };
        let options = TemplateOptions {
            allow_context: true,
            preserve_context_tokens: false,
            preserve_unknown_context: true,
        };
        let expansion = expand_template_string(&host, test_case.text, options);
        assert_eq!(
            (expansion, host.reads.into_inner()),
            (
                test_case.expected,
                test_case
                    .expected_reads
                    .iter()
                    .map(|read| (*read).to_owned())
                    .collect::<Vec<_>>()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_malformed_nested_templates_when_finding_templates_then_the_scan_stays_linear() {
    let test_cases: [TemplateScanTestCase; 3] = [
        TemplateScanTestCase {
            description: "an unclosed chain of nested templates with alternating quotes",
            text: format!("${{'{}", "${\"${'".repeat(4_000)),
            expected_max_steps_per_character: 20,
        },
        TemplateScanTestCase {
            description: "many unclosed templates that each open a quote",
            text: "${'a ".repeat(20_000),
            expected_max_steps_per_character: 20,
        },
        TemplateScanTestCase {
            description: "many sibling templates inside one quoted argument",
            text: format!("${{coalesce('{}')}}", "${env}".repeat(20_000)),
            expected_max_steps_per_character: 20,
        },
    ];
    for test_case in test_cases {
        let (_, steps) = counted_template_spans(&test_case.text);
        assert!(
            steps <= test_case.expected_max_steps_per_character * test_case.text.len(),
            "{}: {steps} steps over {} characters",
            test_case.description,
            test_case.text.len()
        );
    }
}
