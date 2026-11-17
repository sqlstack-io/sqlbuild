"""Exercise the planner's warehouse-reads boundary through Fensu's real Rust analyzer."""

import subprocess
from pathlib import Path

import pytest

from tests.integration.scripts.fensu_policy._test_types import PlannerReadsTestCase
from tests.integration.scripts.fensu_policy.helpers import check_rust, write_planner_project

_PLANNER: tuple[str, ...] = ("crates/sqlbuild-python/src/planner.rs",)


@pytest.mark.parametrize(
    "test_case",
    [
        PlannerReadsTestCase(
            description="planner reading through reads and writing SQL through its dialect",
            files={
                "planner.rs": (
                    "// The adapter and connection stay behind WarehouseReads.\n"
                    "pub fn plan(reads: &Reads, dialect: &Dialect) {\n"
                    '    call(&[("adapter", dialect.clone())]);\n'
                    "    reads.relations();\n"
                    "}\n"
                ),
            },
            planner_paths=_PLANNER,
            expected_returncode=0,
            expected_faults=(),
        ),
        PlannerReadsTestCase(
            description="planner taking the adapter",
            files={"planner.rs": "pub fn plan() {}\npub fn list(adapter: &Any) {}\n"},
            planner_paths=_PLANNER,
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:2:0",),
        ),
        PlannerReadsTestCase(
            description="planner reading the runtime's adapter by name",
            files={
                "planner.rs": 'pub fn plan(runtime: &Any) {\n    runtime.getattr("adapter");\n}\n'
            },
            planner_paths=_PLANNER,
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:2:0",),
        ),
        PlannerReadsTestCase(
            description="planner calling an adapter warehouse method by name",
            files={
                "planner.rs": 'pub fn plan(value: &Any) {\n    field(value, "list_relations");\n}\n'
            },
            planner_paths=_PLANNER,
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:2:0",),
        ),
        PlannerReadsTestCase(
            description="code outside the planner taking the adapter",
            files={
                "planner.rs": "pub fn plan() {}\n",
                "command.rs": "pub fn run(adapter: &Any, connection: &Any) {}\n",
            },
            planner_paths=_PLANNER,
            expected_returncode=0,
            expected_faults=(),
        ),
        PlannerReadsTestCase(
            description="planner path that matches no file",
            files={"planner.rs": "pub fn plan() {}\n"},
            planner_paths=(*_PLANNER, "crates/sqlbuild-python/src/removed.rs"),
            expected_returncode=1,
            expected_faults=("planner path crates/sqlbuild-python/src/removed.rs matches no",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planner_code_when_checking_then_only_warehouse_access_outside_reads_faults(
    tmp_path: Path, test_case: PlannerReadsTestCase
) -> None:
    write_planner_project(
        root=tmp_path, planner_paths=test_case.planner_paths, files=test_case.files
    )

    result: subprocess.CompletedProcess[str] = check_rust(root=tmp_path)

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert result.stdout.count("XSB080") == len(test_case.expected_faults), result.stdout
    assert all(fault in result.stdout for fault in test_case.expected_faults), result.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
