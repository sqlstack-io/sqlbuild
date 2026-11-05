use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::System::SystemServices::{JOB_OBJECT_QUERY, JOB_OBJECT_TERMINATE};

use crate::ownership::_helpers::windows::job_name;
use crate::ownership::main::recover_worker::recover_worker;
use crate::ownership::main::stop_worker::stop_worker;
use crate::ownership::tests::helpers::{
    as_foreign_identity, claims, exited_within, open_job, reap, spawn,
};
use crate::ownership::tests::test_types::JobAccessTestCase;

#[test]
fn given_a_running_worker_job_when_another_identity_opens_it_then_query_and_terminate_are_denied() {
    let test_cases = [
        JobAccessTestCase {
            description: "terminate access",
            access: JOB_OBJECT_TERMINATE,
            expected_own_access: Ok(()),
            expected_foreign_access: Err(ERROR_ACCESS_DENIED),
        },
        JobAccessTestCase {
            description: "query access",
            access: JOB_OBJECT_QUERY,
            expected_own_access: Ok(()),
            expected_foreign_access: Err(ERROR_ACCESS_DENIED),
        },
    ];
    for test_case in test_cases {
        let project = tempfile::tempdir().expect("project");
        let claims = claims(project.path());
        let mut worker = spawn(&claims, (Some(0), 0), 60);
        let name = job_name(&claims[0]);
        assert_eq!(
            open_job(&name, test_case.access),
            test_case.expected_own_access,
            "{}",
            test_case.description
        );
        assert_eq!(
            as_foreign_identity(|| open_job(&name, test_case.access)),
            test_case.expected_foreign_access,
            "{}",
            test_case.description
        );
        assert!(
            !exited_within(&mut worker, std::time::Duration::from_millis(300)),
            "{}",
            test_case.description
        );
        stop_worker(&recover_worker(&claims[0]).expect("recover"), true).expect("stop");
        assert!(
            exited_within(&mut worker, std::time::Duration::from_secs(5)),
            "{}",
            test_case.description
        );
        reap(worker);
    }
}
