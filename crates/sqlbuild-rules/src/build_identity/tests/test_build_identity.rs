use std::process::Command;

use crate::build_identity::constants::MISSING_BUILD_IDENTITY;
use crate::build_identity::errors::BuildIdentityError;
use crate::build_identity::main::install_build_identity::install_build_identity;
use crate::build_identity::tests::helpers::{
    FRESH_PROCESS_ENV_VAR, read_identity_in_fresh_process,
};
use crate::build_identity::tests::test_types;
use crate::tests::helpers::TEST_BUILD_IDENTITY;

#[test]
#[ignore = "runs only in the fresh process the missing-identity test starts"]
fn given_a_fresh_process_when_reading_the_identity_then_the_read_panics() {
    let test_cases = [test_types::FreshProcessTestCase {
        description: "nothing installed an identity in this process",
        expected_installed: false,
    }];
    for test_case in test_cases {
        assert_eq!(
            read_identity_in_fresh_process(),
            test_case.expected_installed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_no_installed_identity_when_reading_it_then_the_read_fails_with_the_reason()
-> Result<(), String> {
    let test_cases = [test_types::MissingIdentityTestCase {
        description: "reading before installing fails with the reason instead of falling back",
        fresh_process_test: "build_identity::tests::build_identity::given_a_fresh_process_when_reading_the_identity_then_the_read_panics",
        expected_failed: "1 failed",
        expected_reason: MISSING_BUILD_IDENTITY,
    }];
    for test_case in test_cases {
        let output = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .args([
                test_case.fresh_process_test,
                "--exact",
                "--ignored",
                "--test-threads=1",
            ])
            .env(FRESH_PROCESS_ENV_VAR, "1")
            .output()
            .map_err(|error| error.to_string())?;
        let printed = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            !output.status.success()
                && printed.contains(test_case.expected_failed)
                && printed.contains(test_case.expected_reason),
            "{}: {printed}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_an_identity_when_installing_then_only_a_non_empty_matching_identity_is_accepted() {
    let test_cases = [
        test_types::InstallTestCase {
            description: "an empty identity would match builds of any sources",
            identity: "",
            expected_result: Err(BuildIdentityError::Empty),
        },
        test_types::InstallTestCase {
            description: "the test identity is installed, again harmlessly when another test did",
            identity: TEST_BUILD_IDENTITY,
            expected_result: Ok(()),
        },
        test_types::InstallTestCase {
            description: "a second, different identity is refused",
            identity: "fedcba9876543210",
            expected_result: Err(BuildIdentityError::Conflicting {
                installed: TEST_BUILD_IDENTITY.to_owned(),
                offered: "fedcba9876543210".to_owned(),
            }),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            install_build_identity(test_case.identity),
            test_case.expected_result,
            "{}",
            test_case.description
        );
    }
}
