"""The build scheduler keeps each named job within its concurrency without holding up other jobs."""

from __future__ import annotations

import pytest

from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    JobLimitedBuildTestCase,
    SubworkerReservationTestCase,
)
from tests.unit.src.sqlbuild.executor.build.classes.helpers import (
    reserve_subworkers,
    run_job_limited_build,
)


@pytest.mark.parametrize(
    "test_case",
    [
        JobLimitedBuildTestCase(
            description="a full job lets ready nodes of other work start",
            upstream={"orders_a": (), "orders_b": (), "orders_c": (), "refunds": (), "crm": ()},
            node_jobs={
                "orders_a": ("orders_job",),
                "orders_b": ("orders_job",),
                "orders_c": ("orders_job",),
            },
            job_limits={"orders_job": 1},
            max_concurrency=4,
            failing=frozenset(),
            expected_peaks={"orders_job": 1, "*": 3},
            expected_skipped=(),
        ),
        JobLimitedBuildTestCase(
            description="a node in two jobs fits within the lower limit",
            upstream={"shared": (), "orders_a": (), "orders_b": (), "refunds_a": ()},
            node_jobs={
                "shared": ("orders_job", "refunds_job"),
                "orders_a": ("orders_job",),
                "orders_b": ("orders_job",),
                "refunds_a": ("refunds_job",),
            },
            job_limits={"orders_job": 2, "refunds_job": 1},
            max_concurrency=4,
            failing=frozenset(),
            expected_peaks={"orders_job": 2, "refunds_job": 1},
            expected_skipped=(),
        ),
        JobLimitedBuildTestCase(
            description="a failed node frees its slot and its dependents are skipped",
            upstream={
                "orders_a": (),
                "orders_b": (),
                "orders_mart": ("orders_a",),
                "orders_report": ("orders_mart",),
            },
            node_jobs={
                "orders_a": ("orders_job",),
                "orders_b": ("orders_job",),
                "orders_mart": ("orders_job",),
                "orders_report": ("orders_job",),
            },
            job_limits={"orders_job": 1},
            max_concurrency=4,
            failing=frozenset({"orders_a"}),
            expected_peaks={"orders_job": 1, "*": 1},
            expected_skipped=("orders_mart", "orders_report"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_job_limits_when_running_the_dispatch_loop_then_each_job_stays_within_its_limit(
    test_case: JobLimitedBuildTestCase,
) -> None:
    assert run_job_limited_build(test_case) == (
        test_case.expected_peaks,
        test_case.expected_skipped,
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        SubworkerReservationTestCase(
            description="sub-workers count against the coordinator's job",
            job_limits={"events_job": 2},
            job_running={"events_job": 1},
            coordinator_jobs=("events_job",),
            queued_coordinator_jobs=(),
            max_concurrency=8,
            expected_demand=1,
            expected_reservations=(True, False),
        ),
        SubworkerReservationTestCase(
            description="a queued coordinator whose job is full claims no sub-worker slots",
            job_limits={"events_job": 3, "refunds_job": 1},
            job_running={"events_job": 1, "refunds_job": 1},
            coordinator_jobs=("events_job",),
            queued_coordinator_jobs=(("refunds_job",), ("refunds_job",)),
            max_concurrency=3,
            expected_demand=1,
            expected_reservations=(True, True, False),
        ),
        SubworkerReservationTestCase(
            description="a queued coordinator that could start keeps its slot",
            job_limits={"events_job": 3},
            job_running={"events_job": 1},
            coordinator_jobs=("events_job",),
            queued_coordinator_jobs=((),),
            max_concurrency=3,
            expected_demand=2,
            expected_reservations=(True, False),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_coordinators_when_reserving_sub_workers_then_job_limits_apply(
    test_case: SubworkerReservationTestCase,
) -> None:
    assert reserve_subworkers(test_case) == (
        test_case.expected_demand,
        test_case.expected_reservations,
    ), test_case.description
