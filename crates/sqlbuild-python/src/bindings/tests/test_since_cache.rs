use std::path::PathBuf;

use sqlbuild_cli::launcher::main::install_native_build_identity::install_native_build_identity;

use crate::bindings::tests::helpers::{cache_key_under, project_reading_environment};
use crate::bindings::tests::test_types::CacheKeyTestCase;

const BASE_ENVIRONMENT: [(&str, &str); 2] = [("ORDERS_REGION", "eu"), ("UNREAD_SETTING", "a")];

#[test]
fn given_referenced_environment_variable_when_keying_base_cache_then_its_value_counts() {
    install_native_build_identity().expect("the build identity installs once");
    let project: PathBuf = project_reading_environment();
    let base_key: String = cache_key_under(&project, &BASE_ENVIRONMENT);
    let test_cases = [
        CacheKeyTestCase {
            description: "a referenced variable with a new value changes the key",
            changed: &[("ORDERS_REGION", "us"), ("UNREAD_SETTING", "a")],
            expected_same_key: false,
        },
        CacheKeyTestCase {
            description: "an unset referenced variable changes the key",
            changed: &[("UNREAD_SETTING", "a")],
            expected_same_key: false,
        },
        CacheKeyTestCase {
            description: "a variable the project does not read leaves the key alone",
            changed: &[("ORDERS_REGION", "eu"), ("UNREAD_SETTING", "b")],
            expected_same_key: true,
        },
    ];
    for test_case in test_cases {
        let changed_key: String = cache_key_under(&project, test_case.changed);

        assert_eq!(
            changed_key == base_key,
            test_case.expected_same_key,
            "{}",
            test_case.description
        );
    }
}
