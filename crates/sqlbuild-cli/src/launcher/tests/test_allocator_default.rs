use std::ffi::OsString;

use crate::launcher::_helpers::dispatch::allocator_default;
use crate::launcher::tests::test_types::AllocatorDefaultTestCase;

#[test]
fn given_inherited_purge_delay_when_handing_off_then_only_an_unset_one_gets_the_default() {
    let test_cases = [
        AllocatorDefaultTestCase {
            description: "an unset delay gets the compile default",
            inherited: None,
            expected_default: Some(("MIMALLOC_PURGE_DELAY", "100")),
        },
        AllocatorDefaultTestCase {
            description: "the user's delay stands",
            inherited: Some("10"),
            expected_default: None,
        },
        AllocatorDefaultTestCase {
            description: "an empty delay the user set stands",
            inherited: Some(""),
            expected_default: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            allocator_default(test_case.inherited.map(OsString::from)),
            test_case
                .expected_default
                .map(|(name, value)| (OsString::from(name), Some(OsString::from(value)))),
            "{}",
            test_case.description
        );
    }
}
