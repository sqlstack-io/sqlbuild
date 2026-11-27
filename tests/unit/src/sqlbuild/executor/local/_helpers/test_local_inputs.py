"""What a local test relied on from the project, read from SQLQuack's report."""

import pytest

from sqlbuild.executor.local._helpers.local_inputs import relied_on
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import ReliedOnTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        ReliedOnTestCase(
            description="an allowance and an override, each once",
            known_differences=(
                {
                    "name": "hash",
                    "allowed_by": "[testing.local] allow in sqlbuild_project.toml",
                    "construct": "fn:HASH",
                },
                {"name": "user_override", "allowed_by": "default", "construct": "fn:TO_DATE"},
                {
                    "name": "hash",
                    "allowed_by": "[testing.local] allow in sqlbuild_project.toml",
                    "construct": "fn:HASH",
                },
            ),
            expected_relied_on=("HASH (allowed)", "TO_DATE (override)"),
        ),
        ReliedOnTestCase(
            description="known differences SQLQuack allows by default are not the project's",
            known_differences=(
                {"name": "session_timezone_utc", "allowed_by": "default", "construct": "fn:NOW"},
            ),
            expected_relied_on=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_conversion_report_when_reading_it_then_lists_the_projects_allowances_and_overrides(
    test_case: ReliedOnTestCase,
) -> None:
    labels: tuple[str, ...] = relied_on({"known_differences": test_case.known_differences})

    assert labels == test_case.expected_relied_on


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
