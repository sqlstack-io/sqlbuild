use sqlbuild_config::models::{ConfigDate, ConfigDateTime, ConfigTime, ConfigValue};

use crate::compile_context::_helpers::yaml_cache::{decoded_document, encoded_document};
use crate::compile_context::tests::helpers::text;
use crate::compile_context::tests::test_types::DocumentTestCase;

#[test]
fn given_loaded_document_when_stored_and_read_back_then_it_is_unchanged() {
    let date: ConfigDate = ConfigDate {
        year: 2026,
        month: 2,
        day: 28,
    };
    let time: ConfigTime = ConfigTime {
        hour: 23,
        minute: 59,
        second: 58,
        microsecond: 123_456,
    };
    let test_cases = [
        DocumentTestCase {
            description: "empty document",
            expected_truncations_read: 0,
            document: ConfigValue::Null,
        },
        DocumentTestCase {
            description: "sources document with repeated strings",
            expected_truncations_read: 0,
            document: ConfigValue::Map(vec![(
                text("sources"),
                ConfigValue::List(vec![
                    ConfigValue::Map(vec![
                        (text("name"), text("orders")),
                        (text("schema"), text("raw")),
                    ]),
                    ConfigValue::Map(vec![
                        (text("name"), text("customers")),
                        (text("schema"), text("raw")),
                    ]),
                ]),
            )]),
        },
        DocumentTestCase {
            description: "every scalar kind",
            expected_truncations_read: 0,
            document: ConfigValue::List(vec![
                ConfigValue::Bool(true),
                ConfigValue::Bool(false),
                ConfigValue::Integer(-42),
                ConfigValue::BigInteger("123456789012345678901234567890".to_owned()),
                ConfigValue::Float(-0.5),
                ConfigValue::Date(date),
                ConfigValue::Time(time),
                ConfigValue::DateTime(ConfigDateTime {
                    date,
                    time,
                    utc_offset_seconds: Some(-3600),
                }),
                ConfigValue::DateTime(ConfigDateTime {
                    date,
                    time,
                    utc_offset_seconds: None,
                }),
                text(""),
                text("unicode ✓"),
            ]),
        },
        DocumentTestCase {
            description: "mapping with non-string keys",
            expected_truncations_read: 0,
            document: ConfigValue::Map(vec![
                (ConfigValue::Integer(1), text("one")),
                (ConfigValue::Null, ConfigValue::List(Vec::new())),
            ]),
        },
    ];
    for test_case in test_cases {
        let stored: Vec<u8> = encoded_document(&test_case.document);

        let decoded: Option<ConfigValue> = decoded_document(&stored);
        let truncations_read: usize = (0..stored.len())
            .filter(|length| {
                decoded_document(&stored[..*length]).as_ref() == Some(&test_case.document)
            })
            .count();

        assert_eq!(
            decoded.as_ref(),
            Some(&test_case.document),
            "{}",
            test_case.description
        );
        assert_eq!(
            truncations_read, test_case.expected_truncations_read,
            "{}",
            test_case.description
        );
    }
}
