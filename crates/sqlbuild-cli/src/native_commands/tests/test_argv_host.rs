use crate::argv::models::{TypeFailure, Value};
use crate::argv::types::ArgvHost;
use crate::native_commands::_helpers::command::argv_host::NativeArgvHost;
use crate::native_commands::tests::test_types::ConversionTestCase;

/// Integer expectations recorded from CPython 3.12 `int(text)`.
#[test]
fn given_converter_when_converting_natively_then_python_conversions_match() {
    let test_cases = [
        ConversionTestCase {
            description: "surrounding ASCII spaces",
            converter: "int",
            text: " 5 ",
            expected_value: Ok(Value::Int("5".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "underscores between digits",
            converter: "int",
            text: "1_0",
            expected_value: Ok(Value::Int("10".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a leading underscore",
            converter: "int",
            text: "_1",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a trailing underscore",
            converter: "int",
            text: "1_",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "doubled underscores",
            converter: "int",
            text: "1__0",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "an Arabic-Indic digit",
            converter: "int",
            text: "\u{663}",
            expected_value: Ok(Value::Int("3".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a non-breaking space",
            converter: "int",
            text: "\u{a0}5",
            expected_value: Ok(Value::Int("5".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "an ASCII separator is not stripped",
            converter: "int",
            text: "\u{1c}5",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a zero-width space is not a space",
            converter: "int",
            text: "\u{200b}7",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "negative zero",
            converter: "int",
            text: "-0",
            expected_value: Ok(Value::Int("0".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a sign and leading zeros",
            converter: "int",
            text: "+007",
            expected_value: Ok(Value::Int("7".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "beyond 64 bits",
            converter: "int",
            text: "-123456789012345678901234567890",
            expected_value: Ok(Value::Int("-123456789012345678901234567890".to_owned())),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a decimal point",
            converter: "int",
            text: "1.5",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "empty text",
            converter: "int",
            text: "",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a positive integer rejects zero with its message",
            converter: "_positive_integer",
            text: "0",
            expected_value: Err(TypeFailure::ArgumentType(
                "value must be a positive integer".to_owned(),
            )),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a positive integer rejects negatives",
            converter: "_positive_integer",
            text: "-3",
            expected_value: Err(TypeFailure::ArgumentType(
                "value must be a positive integer".to_owned(),
            )),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "a positive integer that is not an integer is invalid",
            converter: "_positive_integer",
            text: "many",
            expected_value: Err(TypeFailure::Invalid),
            expected_needs_python: false,
        },
        ConversionTestCase {
            description: "--vars JSON needs Python",
            converter: "parse_cli_vars",
            text: "{}",
            expected_value: Ok(Value::None),
            expected_needs_python: true,
        },
    ];

    for test_case in test_cases {
        let mut host: NativeArgvHost = NativeArgvHost::default();

        let value = host.convert(test_case.converter, test_case.text);

        assert_eq!(
            (value, host.needs_python),
            (test_case.expected_value, test_case.expected_needs_python),
            "{}",
            test_case.description
        );
    }
}
