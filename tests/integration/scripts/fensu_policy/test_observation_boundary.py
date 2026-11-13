"""Exercise the observation and synced-copy boundaries through Fensu's real Rust analyzer."""

import subprocess
from pathlib import Path

import pytest

from tests.integration.scripts.fensu_policy._test_types import ObservationBoundaryTestCase
from tests.integration.scripts.fensu_policy.helpers import check_rust, write_observation_project


@pytest.mark.parametrize(
    "test_case",
    [
        ObservationBoundaryTestCase(
            description="planner mentioning observations only in a comment",
            rule="XSB078",
            allowed=("crates/sqlbuild-python/src/observations",),
            planner="// OBSERVATIONS_TABLE_NAME is display-only\npub fn plan() {}\n",
            expected_returncode=0,
            expected_faults=(),
        ),
        ObservationBoundaryTestCase(
            description="planner matching the observations history kind",
            rule="XSB078",
            allowed=("crates/sqlbuild-python/src/observations",),
            planner=(
                "pub fn plan() {}\n"
                "pub fn mirrored(kind: Kind) -> bool { kind == WarehouseHistoryKind::Observations }\n"
            ),
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:2:0",),
        ),
        ObservationBoundaryTestCase(
            description="planner reading the observation table by its SQL name",
            rule="XSB078",
            allowed=("crates/sqlbuild-python/src/observations",),
            planner='pub const READ: &str = "SELECT value FROM _sqlbuild_observations";\n',
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:1:0",),
        ),
        ObservationBoundaryTestCase(
            description="allowed path that no longer names observation state",
            rule="XSB078",
            allowed=(
                "crates/sqlbuild-python/src/observations",
                "crates/sqlbuild-python/src/planner.rs",
            ),
            planner="pub fn plan() {}\n",
            expected_returncode=1,
            expected_faults=("allowed path crates/sqlbuild-python/src/planner.rs no longer names",),
        ),
        ObservationBoundaryTestCase(
            description="live planner naming the synced copy only in a comment",
            rule="XSB079",
            allowed=("crates/sqlbuild-python/src/observations",),
            planner="// SyncedCopyAdapter serves plan previews only\npub fn plan() {}\n",
            expected_returncode=0,
            expected_faults=(),
        ),
        ObservationBoundaryTestCase(
            description="live planner branching on the synced copy",
            rule="XSB079",
            allowed=("crates/sqlbuild-python/src/observations",),
            planner=(
                "pub fn plan() {}\n"
                "pub fn preview(adapter: &Any) -> bool { adapter.is::<SyncedCopyAdapter>() }\n"
            ),
            expected_returncode=1,
            expected_faults=(" --> crates/sqlbuild-python/src/planner.rs:2:0",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_naming_preview_only_state_when_checking_then_only_unallowed_code_faults(
    tmp_path: Path, test_case: ObservationBoundaryTestCase
) -> None:
    write_observation_project(
        root=tmp_path, rule=test_case.rule, allowed=test_case.allowed, planner=test_case.planner
    )

    result: subprocess.CompletedProcess[str] = check_rust(root=tmp_path)

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert result.stdout.count(test_case.rule) == len(test_case.expected_faults), result.stdout
    assert all(fault in result.stdout for fault in test_case.expected_faults), result.stdout
    assert "src/observations/mod.rs" not in result.stdout
