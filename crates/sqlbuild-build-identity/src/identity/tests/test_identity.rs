use crate::identity::main::native_build_identity::native_build_identity;
use crate::identity::tests::test_types;

#[test]
fn given_native_build_when_reading_cache_identity_then_it_is_a_source_hash() {
    let test_cases = [test_types::BuildIdentityTestCase {
        description: "cache identity is a source content hash",
        expected_hash_length: 16,
    }];
    for test_case in &test_cases {
        let hash = native_build_identity();

        assert!(
            hash.len() == test_case.expected_hash_length
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_workspace_checkout_when_deriving_build_identity_then_workspace_files_are_hashed() {
    let test_cases = [
        test_types::HashedWorkspaceFileTestCase {
            description: "dependency features and build profiles change the identity",
            workspace_file: "../../Cargo.toml",
            expected_hashed: true,
        },
        test_types::HashedWorkspaceFileTestCase {
            description: "resolved dependency versions change the identity",
            workspace_file: "../../Cargo.lock",
            expected_hashed: true,
        },
    ];
    for test_case in &test_cases {
        let hashed = env!("SQLBUILD_NATIVE_HASHED_WORKSPACE_FILES")
            .split(',')
            .any(|file| file == test_case.workspace_file);
        assert_eq!(
            hashed, test_case.expected_hashed,
            "{}",
            test_case.description
        );
    }
}
