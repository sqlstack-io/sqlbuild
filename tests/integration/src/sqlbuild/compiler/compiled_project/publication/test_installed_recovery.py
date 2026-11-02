"""An installed native executable retains offline administrator recovery guarantees."""

import shutil
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    InstalledRecoveryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.helpers import (
    verify_recovery,
)


@pytest.mark.parametrize(
    "test_case",
    [
        InstalledRecoveryTestCase(
            description="installed native server recovers authority without rebootstrap",
            executable="sqb",
            expected_rejection="cannot remove or disable the final usable installation administrator",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_installed_server_when_recovered_offline_then_authority_is_restored_without_rebootstrap(
    test_case: InstalledRecoveryTestCase,
) -> None:
    binary: str | None = shutil.which(test_case.executable)
    assert binary is not None
    assert test_case.expected_rejection in verify_recovery(Path(binary))
