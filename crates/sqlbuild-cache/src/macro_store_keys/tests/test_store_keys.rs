use sqlbuild_core::json::models::JsonValue;

use crate::macro_store_keys::main::canonical_sort_text::canonical_sort_text;
use crate::macro_store_keys::main::context_store_token::context_store_token;
use crate::macro_store_keys::main::macro_store_token::macro_store_token;
use crate::macro_store_keys::tests::helpers::{array, text};
use crate::macro_store_keys::tests::test_types::StoreKeyTestCase;

#[test]
fn given_macro_identity_when_keying_then_matches_python_key_text() {
    let test_cases = [
        StoreKeyTestCase {
            description: "name, path, identity and source digest as compact JSON",
            value: array(vec![text("tuple"), array(vec![text("a")])]),
            expected_key: Some(
                "[\"cents\",\"sql/macros/money.py\",[\"tuple\",[\"a\"]],\"09b7bf2b19ceac419ff40c8cbe965d0c135424c443d286890f788ec6efb5f92d\"]",
            ),
        },
        StoreKeyTestCase {
            description: "a lone surrogate has no key",
            value: JsonValue::Utf16(vec![0xd800]),
            expected_key: None,
        },
    ];

    for test_case in test_cases {
        let key = macro_store_token(
            (text("cents"), text("sql/macros/money.py")),
            test_case.value,
            "SELECT 1",
        )
        .unwrap_or_default();

        assert_eq!(
            key.as_deref(),
            test_case.expected_key,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_canonical_context_when_keying_then_matches_python_digest() {
    let test_cases = [StoreKeyTestCase {
        description: "the digest of the compact context text",
        value: array(vec![
            array(vec![
                text("x.MacroContext"),
                array(vec![array(vec![text("adapter_name"), text("duckdb")])]),
            ]),
            JsonValue::Null,
        ]),
        expected_key: Some("03272a69b9c5e3b3da0067db1a2deab59532aafa30306f624a7c3240eba4740b"),
    }];

    for test_case in test_cases {
        let key = context_store_token(&test_case.value).unwrap_or_default();

        assert_eq!(
            key.as_deref(),
            test_case.expected_key,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_set_member_when_sorting_then_matches_python_json_dumps() {
    let test_cases = [StoreKeyTestCase {
        description: "ASCII-escaped with spaced separators",
        value: array(vec![text("int"), text("\u{e9}")]),
        expected_key: Some("[\"int\", \"\\u00e9\"]"),
    }];

    for test_case in test_cases {
        let key = canonical_sort_text(&test_case.value).ok();

        assert_eq!(
            key.as_deref(),
            test_case.expected_key,
            "{}",
            test_case.description
        );
    }
}
