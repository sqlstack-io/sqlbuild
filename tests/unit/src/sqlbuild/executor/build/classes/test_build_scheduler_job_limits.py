"""The build scheduler keeps each named job within its concurrency without holding up other jobs."""

from __future__ import annotations

import pytest

from tests.unit.src.sqlbuild.executor.build.classes._test_types import JobLimitDispatchTestCase
from tests.unit.src.sqlbuild.executor.build.classes.helpers import dispatched_names


@pytest.mark.parametrize(
    "test_case",
    [
        JobLimitDispatchTestCase(
            description="a full job lets the next ready node of another job start",
            ready=("orders_a", "orders_b", "refunds_a"),
            node_jobs={"orders_a": ("orders_job",), "orders_b": ("orders_job",)},
            job_limits={"orders_job": 1},
            blocked=frozenset(),
            finished_before=("", "", "", "orders_a"),
            expected_dispatched=("orders_a", "refunds_a", None, "orders_b"),
        ),
        JobLimitDispatchTestCase(
            description="a node in two jobs fits within the lower limit",
            ready=("shared", "orders_a", "refunds_a"),
            node_jobs={
                "shared": ("orders_job", "refunds_job"),
                "orders_a": ("orders_job",),
                "refunds_a": ("refunds_job",),
            },
            job_limits={"orders_job": 2, "refunds_job": 1},
            blocked=frozenset(),
            finished_before=("", "", "", "shared"),
            expected_dispatched=("shared", "orders_a", None, "refunds_a"),
        ),
        JobLimitDispatchTestCase(
            description="a node to skip never waits for its job",
            ready=("orders_a", "orders_b"),
            node_jobs={"orders_a": ("orders_job",), "orders_b": ("orders_job",)},
            job_limits={"orders_job": 1},
            blocked=frozenset({"orders_b"}),
            finished_before=("", ""),
            expected_dispatched=("orders_a", "orders_b"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_job_limits_when_dispatching_then_each_job_stays_within_its_limit(
    test_case: JobLimitDispatchTestCase,
) -> None:
    assert dispatched_names(test_case) == test_case.expected_dispatched, test_case.description
