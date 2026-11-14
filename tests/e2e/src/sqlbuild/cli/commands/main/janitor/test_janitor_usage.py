"""E2E tests for the janitor usage check: which archive candidates are in use, and keeping them."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.janitor._test_types import (
    JanitorKeepInUseIncompleteE2ETestCase,
    JanitorUsageE2ETestCase,
    JanitorUsageFailureE2ETestCase,
    JanitorUsageJsonE2ETestCase,
    JanitorUsagePromptE2ETestCase,
    JanitorUsageUncheckedE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.janitor.helpers import (
    assert_in_order,
    count_stale_archives,
    in_use_entries,
    list_archive_names,
    prepare_probe_janitor_project,
    prepare_usage_janitor_project,
    read_usage_calls,
    write_usage_settings,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    run_sqb,
    run_sqb_with_pty,
    table_exists,
)

APPROVED_JANITOR: tuple[str, ...] = ("--no-color", "janitor", "--auto-approve")
FIVE_READERS: tuple[str, ...] = ("svc_reporting", "alice", "bob", "carol", "dave")


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsageUncheckedE2ETestCase(
            description="an adapter without a usage source says so once and archives as before",
            janitor_command=APPROVED_JANITOR,
            expected_ordered_lines=(
                "Inspected warehouse state. (",
                "Checking relation usage...",
                "Relation usage not checked: unsupported adapter. (",
                "Janitor preview  default",
                "  relations to archive   3\n",
                "\n  usage not checked: unsupported adapter\n",
                "Relations to archive",
                "\u2713 Janitor completed  archived 3, deleted 1, pruned 2",
            ),
            expected_archived=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_when_running_janitor_then_usage_is_reported_not_checked(
    test_case: JanitorUsageUncheckedE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path, project_name="janitor_usage_unchecked", stale_tables={"main": 3}
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert_in_order(output=result.stdout, lines=test_case.expected_ordered_lines)
    assert result.stdout.count("usage not checked: unsupported adapter\n") == 1
    assert "In use" not in result.stdout
    assert count_stale_archives(db_path=project_dir / "janitor.duckdb") == (
        test_case.expected_archived
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsageJsonE2ETestCase(
            description="duckdb reports usage as not checked and every candidate's in_use null",
            janitor_command=("janitor", "--auto-approve", "--json"),
            expected_usage_check={
                "status": "not_checked",
                "reason": "unsupported adapter",
                "window": "7d",
                "keep_in_use": False,
                "notices": [],
            },
            expected_archive_in_use=(("main.old_orders_000", None), ("main.old_orders_001", None)),
            expected_kept_in_use=(),
            expected_archived=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_when_running_janitor_json_then_usage_check_is_not_checked(
    test_case: JanitorUsageJsonE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path, project_name="janitor_usage_json", stale_tables={"main": 2}
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["usage_check"] == test_case.expected_usage_check
    assert in_use_entries(payload["plan"]["relations_to_archive"]) == (
        test_case.expected_archive_in_use
    )
    assert in_use_entries(payload["plan"]["kept_in_use"]) == test_case.expected_kept_in_use
    assert payload["result"]["counts"]["archived"] == test_case.expected_archived
    assert "usage not checked: unsupported adapter" in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsageE2ETestCase(
            description="in-use relations are listed with readers and archived by default",
            janitor_config="",
            janitor_command=APPROVED_JANITOR,
            reads={
                "old_orders_000": (FIVE_READERS, 40),
                "old_orders_view": (("svc_reporting",), 14),
            },
            expected_ordered_lines=(
                "Checking relation usage...",
                "Checked relation usage: 2 of 4 in use. (",
                "  relations to archive   4\n  in use                 2\n",
                "Relations to archive",
                "\nIn use\n",
                "  main.old_orders_000  queried 2026-10-21 by svc_reporting, alice, bob +2 more "
                "(40 queries in 7 days); frozen since 2026-09-01, still in use\n",
                "  main.old_orders_view  queried 2026-10-21 by svc_reporting "
                "(14 queries in 7 days)\n",
                "\u2713 Janitor completed  archived 4, deleted 1, pruned 2",
            ),
            unexpected_fragments=(
                "Kept (in use)",
                "old_orders_view  queried 2026-10-21 by svc_reporting (14 queries in 7 days);",
                "old_orders_001  queried",
                "usage not checked",
            ),
            expected_remaining=(),
            expected_archived=4,
        ),
        JanitorUsageE2ETestCase(
            description="--keep-in-use keeps in-use relations out of the archive and the counts",
            janitor_config="",
            janitor_command=(*APPROVED_JANITOR, "--keep-in-use"),
            reads={
                "old_orders_000": (FIVE_READERS, 40),
                "old_orders_view": (("svc_reporting",), 14),
            },
            expected_ordered_lines=(
                "Checked relation usage: 2 of 4 in use. (",
                "  relations to archive   2\n  kept (in use)          2\n",
                "\nKept (in use)\n  main.old_orders_000  queried 2026-10-21",
                "\u2713 Janitor completed  archived 2, deleted 1, pruned 2",
            ),
            unexpected_fragments=(
                "\nIn use\n",
                "main.old_orders_000  ->",
                "main.old_orders_view  ->",
                "archived  main.old_orders_000",
            ),
            expected_remaining=("old_orders_000", "old_orders_view"),
            expected_archived=2,
        ),
        JanitorUsageE2ETestCase(
            description="a relation read only through views names them",
            janitor_config="",
            janitor_command=APPROVED_JANITOR,
            reads={"old_orders_002": (("alice",), 3)},
            via={"old_orders_002": ("main.orders_report", "main.orders_summary")},
            expected_ordered_lines=(
                "Checked relation usage: 1 of 4 in use. (",
                "  main.old_orders_002  queried 2026-10-21 by alice via main.orders_report, "
                "main.orders_summary (3 queries in 7 days); frozen since 2026-09-01, "
                "still in use\n",
            ),
            unexpected_fragments=("Kept (in use)",),
            expected_remaining=(),
            expected_archived=4,
        ),
        JanitorUsageE2ETestCase(
            description="keep_in_use in the project and in_use_window reach the usage source",
            janitor_config='keep_in_use = true\nin_use_window = "36h"\n',
            janitor_command=APPROVED_JANITOR,
            reads={"old_orders_001": (("alice",), 1)},
            expected_ordered_lines=(
                "  kept (in use)          1\n",
                "  main.old_orders_001  queried 2026-10-21 by alice (1 query in 36 hours); "
                "frozen since 2026-09-01, still in use\n",
                "\u2713 Janitor completed  archived 3, deleted 1, pruned 2",
            ),
            unexpected_fragments=("main.old_orders_001  ->",),
            expected_remaining=("old_orders_001",),
            expected_archived=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_usage_source_when_running_janitor_then_in_use_relations_are_shown_or_kept(
    test_case: JanitorUsageE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_usage_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_usage",
        stale_tables=3,
        stale_views=("old_orders_view",),
        janitor_config=test_case.janitor_config,
    )
    write_usage_settings(project_dir=project_dir, reads=test_case.reads, via=test_case.via)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    db_path: Path = project_dir / "janitor.duckdb"
    assert result.returncode == 0, result.stdout + result.stderr
    assert_in_order(output=result.stdout, lines=test_case.expected_ordered_lines)
    fragment: str
    for fragment in test_case.unexpected_fragments:
        assert fragment not in result.stdout
    name: str
    for name in test_case.expected_remaining:
        assert table_exists(db_path=db_path, table_name=name)
    assert count_stale_archives(db_path=db_path) == test_case.expected_archived
    calls: dict[str, object] = read_usage_calls(project_dir=project_dir)
    assert calls["relations"] == [
        "old_orders_000",
        "old_orders_001",
        "old_orders_002",
        "old_orders_view",
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsageJsonE2ETestCase(
            description="kept and archived candidates carry their reads, or false when unread",
            janitor_command=("janitor", "--auto-approve", "--json", "--keep-in-use"),
            expected_usage_check={
                "status": "checked",
                "reason": None,
                "window": "7d",
                "keep_in_use": True,
                "notices": [],
            },
            expected_archive_in_use=(("main.old_orders_001", False),),
            expected_kept_in_use=(
                (
                    "main.old_orders_000",
                    {
                        "last_read": "2026-10-21T09:30:00Z",
                        "readers": list(FIVE_READERS),
                        "query_count": 40,
                        "via": None,
                    },
                ),
                (
                    "main.old_orders_002",
                    {
                        "last_read": "2026-10-21T09:30:00Z",
                        "readers": ["alice"],
                        "query_count": 2,
                        "via": ["main.orders_report"],
                    },
                ),
            ),
            expected_archived=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_usage_source_when_janitor_prints_json_then_candidates_carry_in_use(
    test_case: JanitorUsageJsonE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_usage_janitor_project(
        tmp_path=tmp_path, project_name="janitor_usage_json_shape", stale_tables=3
    )
    write_usage_settings(
        project_dir=project_dir,
        reads={"old_orders_000": (FIVE_READERS, 40), "old_orders_002": (("alice",), 2)},
        via={"old_orders_002": ("main.orders_report",)},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["usage_check"] == test_case.expected_usage_check
    assert in_use_entries(payload["plan"]["relations_to_archive"]) == (
        test_case.expected_archive_in_use
    )
    assert in_use_entries(payload["plan"]["kept_in_use"]) == test_case.expected_kept_in_use
    assert all(
        set(entry) == {"name", "age_days", "in_use"} for entry in payload["plan"]["kept_in_use"]
    )
    assert payload["result"]["counts"]["archived"] == test_case.expected_archived
    assert read_usage_calls(project_dir=project_dir)["window_seconds"] == 7 * 86_400


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsagePromptE2ETestCase(
            description="the prompt states how many relations to archive are in use",
            janitor_command=("--no-color", "janitor"),
            reads={"old_orders_000": (("alice",), 3)},
            unavailable=None,
            expected_fragments=(
                "Janitor will archive 3 and delete 3 items from default.\r\n"
                "3 relations will be archived; 1 of them is in use.\r\n",
                "Janitor cancelled.",
            ),
        ),
        JanitorUsagePromptE2ETestCase(
            description="the prompt repeats why usage was not checked",
            janitor_command=("--no-color", "janitor"),
            reads={},
            unavailable="cannot read access history",
            expected_fragments=(
                "\r\nUsage not checked: cannot read access history\r\nRetention: 14 days",
                "Janitor cancelled.",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_usage_check_when_prompting_then_prompt_states_usage(
    test_case: JanitorUsagePromptE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_usage_janitor_project(
        tmp_path=tmp_path, project_name="janitor_usage_prompt", stale_tables=3
    )
    write_usage_settings(
        project_dir=project_dir, reads=test_case.reads, unavailable=test_case.unavailable
    )

    result: subprocess.CompletedProcess[str] = run_sqb_with_pty(
        command=test_case.janitor_command, project_dir=project_dir, input_text="no\n"
    )

    assert result.returncode == 1, result.stdout
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
    assert len(list_archive_names(db_path=project_dir / "janitor.duckdb")) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUsageFailureE2ETestCase(
            description="a failed usage read prints a failure line and archives nothing",
            failure="usage warehouse went away",
            expected_ordered_lines=(
                "Checking relation usage...",
                "Relation usage check failed. (",
                "usage warehouse went away",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_usage_source_when_running_janitor_then_failure_line_is_printed(
    test_case: JanitorUsageFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_usage_janitor_project(
        tmp_path=tmp_path, project_name="janitor_usage_failure", stale_tables=1
    )
    write_usage_settings(project_dir=project_dir, failure=test_case.failure)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=project_dir
    )

    assert result.returncode != 0, result.stdout
    assert_in_order(output=result.stdout + result.stderr, lines=test_case.expected_ordered_lines)
    assert "Janitor preview" not in result.stdout
    assert len(list_archive_names(db_path=project_dir / "janitor.duckdb")) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorKeepInUseIncompleteE2ETestCase(
            description="--keep-in-use stops when usage cannot be read",
            reads={},
            unavailable="cannot read access history",
            notices=(),
            expected_error=(
                "keep_in_use needs the usage of every relation to archive, but usage was not "
                "checked: cannot read access history; nothing was archived"
            ),
        ),
        JanitorKeepInUseIncompleteE2ETestCase(
            description="--keep-in-use stops when usage is only partly read",
            reads={"old_orders_000": (("alice",), 3)},
            unavailable=None,
            notices=("view chains not followed: grant the dependency role",),
            expected_error=(
                "keep_in_use needs the usage of every relation to archive, but usage was only "
                "partly checked: view chains not followed: grant the dependency role; nothing "
                "was archived"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_incomplete_usage_when_keeping_in_use_then_janitor_stops_and_archives_nothing(
    test_case: JanitorKeepInUseIncompleteE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_usage_janitor_project(
        tmp_path=tmp_path, project_name="janitor_keep_incomplete", stale_tables=2
    )
    write_usage_settings(
        project_dir=project_dir,
        reads=test_case.reads,
        unavailable=test_case.unavailable,
        notices=test_case.notices,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(*APPROVED_JANITOR, "--keep-in-use"), project_dir=project_dir
    )

    assert result.returncode != 0, result.stdout
    output: str = " ".join((result.stdout + result.stderr).split())
    assert test_case.expected_error in output, output
    assert "Relation usage check incomplete. (" in result.stdout + result.stderr
    assert "Janitor completed" not in result.stdout
    assert len(list_archive_names(db_path=project_dir / "janitor.duckdb")) == 1


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
