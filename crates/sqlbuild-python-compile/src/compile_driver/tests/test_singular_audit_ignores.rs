use crate::compile_driver::_helpers::jobs::job_node_settings::singular_audit_ignores;
use crate::compile_driver::tests::test_types::SingularAuditIgnoresTestCase;

#[test]
fn given_a_singular_audit_when_its_nodes_have_job_and_build_values_then_all_must_ignore() {
    let test_cases = [
        SingularAuditIgnoresTestCase {
            description: "a job node ignores, a node outside the jobs takes the build's false",
            job_values: vec![Some(true), None],
            build_ignores: false,
            expected_ignores: Some(false),
        },
        SingularAuditIgnoresTestCase {
            description: "a job node ignores, a node outside the jobs takes the build's true",
            job_values: vec![Some(true), None],
            build_ignores: true,
            expected_ignores: Some(true),
        },
        SingularAuditIgnoresTestCase {
            description: "a job node that refuses wins over the build's true",
            job_values: vec![Some(false), None],
            build_ignores: true,
            expected_ignores: Some(false),
        },
        SingularAuditIgnoresTestCase {
            description: "two job nodes that both ignore",
            job_values: vec![Some(true), Some(true)],
            build_ignores: false,
            expected_ignores: Some(true),
        },
        SingularAuditIgnoresTestCase {
            description: "two job nodes that disagree",
            job_values: vec![Some(true), Some(false)],
            build_ignores: true,
            expected_ignores: Some(false),
        },
        SingularAuditIgnoresTestCase {
            description: "no read node has a job value, so the build decides",
            job_values: vec![None, None],
            build_ignores: true,
            expected_ignores: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            singular_audit_ignores(test_case.job_values.into_iter(), test_case.build_ignores),
            test_case.expected_ignores,
            "{}",
            test_case.description
        );
    }
}
