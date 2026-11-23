use crate::tree::tests::helpers::junction_walk;
use crate::tree::tests::test_types::JunctionWalkTestCase;

#[test]
fn given_a_junction_when_walking_then_it_is_walked_like_python_walks_a_directory() {
    let test_cases = [JunctionWalkTestCase {
        description: "a junction is a real directory to Python's os.scandir",
        expected_paths: &["models/linked/orders.sql"],
    }];
    for test_case in test_cases {
        assert_eq!(
            junction_walk(),
            test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}
